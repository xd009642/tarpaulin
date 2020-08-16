use crate::source_analysis::prelude::*;
use quote::ToTokens;
use std::path::PathBuf;
use syn::{spanned::Spanned, *};

impl SourceAnalysis {
    pub(crate) fn process_items(&mut self, items: &[Item], ctx: &Context) -> SubResult {
        let mut res = SubResult::Ok;
        for item in items.iter() {
            match *item {
                Item::ExternCrate(ref i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Use(ref i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Mod(ref i) => self.visit_mod(&i, ctx),
                Item::Fn(ref i) => self.visit_fn(&i, ctx),
                Item::Struct(ref i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Enum(ref i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Union(ref i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Trait(ref i) => self.visit_trait(&i, ctx),
                Item::Impl(ref i) => self.visit_impl(&i, ctx),
                Item::Macro(ref i) => {
                    if let SubResult::Unreachable = self.visit_macro_call(&i.mac, ctx) {
                        res = SubResult::Unreachable;
                    }
                }
                Item::Const(ref c) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(c);
                }
                _ => {}
            }
        }
        res
    }

    fn visit_mod(&mut self, module: &ItemMod, ctx: &Context) {
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        analysis.ignore_tokens(module.mod_token);
        let mut check_insides = true;
        for attr in &module.attrs {
            if let Ok(x) = attr.parse_meta() {
                if check_cfg_attr(&x) {
                    analysis.ignore_tokens(module);
                    if let Some((ref braces, _)) = module.content {
                        analysis.ignore_span(braces.span);
                    }
                    check_insides = false;
                    break;
                } else if ctx.config.ignore_tests && x.path().is_ident("cfg") {
                    if let Meta::List(ref ml) = x {
                        for nested in &ml.nested {
                            if let NestedMeta::Meta(Meta::Path(ref i)) = *nested {
                                if i.is_ident("test") {
                                    check_insides = false;
                                    analysis.ignore_tokens(module.mod_token);
                                    if let Some((ref braces, _)) = module.content {
                                        analysis.ignore_span(braces.span);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if check_insides {
            if let Some((_, ref items)) = module.content {
                self.process_items(items, ctx);
            }
        } else {
            // Get the file or directory name of the module
            let mut p = if let Some(parent) = ctx.file.parent() {
                parent.join(module.ident.to_string())
            } else {
                PathBuf::from(module.ident.to_string())
            };
            if !p.exists() {
                p.set_extension("rs");
            }
            ctx.ignore_mods.borrow_mut().insert(p);
        }
    }

    fn visit_fn(&mut self, func: &ItemFn, ctx: &Context) {
        let mut test_func = false;
        let mut ignored_attr = false;
        let mut is_inline = false;
        let mut ignore_span = false;
        for attr in &func.attrs {
            if let Ok(x) = attr.parse_meta() {
                let id = x.path();
                if id.is_ident("test") {
                    test_func = true;
                } else if id.is_ident("derive") {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_span(attr.bracket_token.span);
                } else if id.is_ident("inline") {
                    is_inline = true;
                } else if id.is_ident("ignore") {
                    ignored_attr = true;
                } else if check_cfg_attr(&x) {
                    ignore_span = true;
                    break;
                }
            }
        }
        if ignore_span
            || (test_func && ctx.config.ignore_tests)
            || (ignored_attr && !ctx.config.run_ignored)
        {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(func);
        } else {
            if is_inline {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                // We need to force cover!
                analysis.cover_span(func.block.brace_token.span, Some(ctx.file_contents));
            }
            if let SubResult::Unreachable = self.process_statements(&func.block.stmts, ctx) {
                // if the whole body of the function is unreachable, that means the function itself
                // cannot be called, so is unreachable as a whole
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(func);
                return;
            }
            self.visit_generics(&func.sig.generics, ctx);
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            let line_number = func.sig.fn_token.span().start().line;
            analysis.ignore.remove(&Lines::Line(line_number));
            // Ignore multiple lines of fn decl
            let decl_start = func.sig.fn_token.span().start().line + 1;
            let stmts_start = func.block.span().start().line;
            let lines = (decl_start..(stmts_start + 1)).collect::<Vec<_>>();
            analysis.add_to_ignore(&lines);
        }
    }

    fn visit_trait(&mut self, trait_item: &ItemTrait, ctx: &Context) {
        let check_cover = self.check_attr_list(&trait_item.attrs, ctx);
        if check_cover {
            for item in &trait_item.items {
                if let TraitItem::Method(ref i) = *item {
                    if self.check_attr_list(&i.attrs, ctx) {
                        if let Some(ref block) = i.default {
                            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                            analysis.cover_token_stream(
                                item.into_token_stream(),
                                Some(ctx.file_contents),
                            );
                            self.visit_generics(&i.sig.generics, ctx);
                            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                            analysis
                                .ignore
                                .remove(&Lines::Line(i.sig.span().start().line));

                            // Ignore multiple lines of fn decl
                            let decl_start = i.sig.fn_token.span().start().line + 1;
                            let stmts_start = block.span().start().line;
                            let lines = (decl_start..(stmts_start + 1)).collect::<Vec<_>>();
                            analysis.add_to_ignore(&lines);
                        }
                    } else {
                        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                        analysis.ignore_tokens(i);
                    }
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    for a in &i.attrs {
                        analysis.ignore_tokens(a);
                    }
                }
            }
            self.visit_generics(&trait_item.generics, ctx);
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(trait_item);
        }
    }

    fn visit_impl(&mut self, impl_blk: &ItemImpl, ctx: &Context) {
        let check_cover = self.check_attr_list(&impl_blk.attrs, ctx);
        if check_cover {
            for item in &impl_blk.items {
                if let ImplItem::Method(ref i) = *item {
                    if self.check_attr_list(&i.attrs, ctx) {
                        {
                            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                            analysis
                                .cover_token_stream(i.into_token_stream(), Some(ctx.file_contents));
                        }
                        if let SubResult::Unreachable = self.process_statements(&i.block.stmts, ctx)
                        {
                            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                            // if the body of this method is unreachable, this means that the method
                            // cannot be called, and is unreachable
                            analysis.ignore_tokens(i);
                            return;
                        }

                        self.visit_generics(&i.sig.generics, ctx);
                        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                        analysis.ignore.remove(&Lines::Line(i.span().start().line));

                        // Ignore multiple lines of fn decl
                        let decl_start = i.sig.fn_token.span().start().line + 1;
                        let stmts_start = i.block.span().start().line;
                        let lines = (decl_start..(stmts_start + 1)).collect::<Vec<_>>();
                        analysis.add_to_ignore(&lines);
                    } else {
                        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                        analysis.ignore_tokens(item);
                    }
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    for a in &i.attrs {
                        analysis.ignore_tokens(a);
                    }
                }
            }
            self.visit_generics(&impl_blk.generics, ctx);
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(impl_blk);
        }
    }
}
