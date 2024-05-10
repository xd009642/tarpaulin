use crate::source_analysis::prelude::*;
use syn::*;

impl SourceAnalysis {
    pub(crate) fn process_items(&mut self, items: &[Item], ctx: &Context) -> SubResult {
        let mut res = SubResult::Ok;
        for item in items.iter() {
            match item {
                Item::ExternCrate(i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Use(i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Mod(i) => self.visit_mod(i, ctx),
                Item::Fn(i) => self.visit_fn(i, ctx, false),
                Item::Struct(i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Enum(i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Union(i) => {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    analysis.ignore_tokens(i);
                }
                Item::Trait(i) => self.visit_trait(i, ctx),
                Item::Impl(i) => self.visit_impl(i, ctx),
                Item::Macro(ref i) => {
                    if self.visit_macro_call(&i.mac, ctx).is_unreachable() {
                        res = SubResult::Unreachable;
                    }
                }
                Item::Const(c) => {
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
        let check_insides = self.check_attr_list(&module.attrs, ctx);
        if check_insides {
            if let Some((_, ref items)) = module.content {
                self.process_items(items, ctx);
            }
        } else {
            if let Some((ref braces, _)) = module.content {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_span(braces.span.join());
            }
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

    fn visit_fn(&mut self, func: &ItemFn, ctx: &Context, force_cover: bool) {
        let mut test_func = false;
        let mut ignored_attr = false;
        let mut is_inline = false;
        let mut ignore_span = false;
        let is_generic = is_sig_generic(&func.sig);
        for attr in &func.attrs {
            let id = attr.path();
            if id.is_ident("test") || id.segments.last().is_some_and(|seg| seg.ident == "test") {
                test_func = true;
            } else if id.is_ident("derive") {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_span(attr.span());
            } else if id.is_ident("inline") {
                is_inline = true;
            } else if id.is_ident("ignore") {
                ignored_attr = true;
            } else if check_cfg_attr(&attr.meta) {
                ignore_span = true;
                break;
            }
        }
        if ignore_span
            || (test_func && !ctx.config.include_tests())
            || (ignored_attr && !ctx.config.run_ignored)
        {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(func);
        } else {
            if is_inline || is_generic || force_cover {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                // We need to force cover!
                analysis.cover_span(func.block.span(), Some(ctx.file_contents));
            }
            if self
                .process_statements(&func.block.stmts, ctx)
                .is_unreachable()
            {
                // if the whole body of the function is unreachable, that means the function itself
                // cannot be called, so is unreachable as a whole
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(func);
                return;
            }
            self.visit_generics(&func.sig.generics, ctx);
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            let line_number = func.sig.fn_token.span().start().line;
            let mut start_line = line_number;
            for attr in &func.attrs {
                start_line = start_line.min(attr.span().start().line);
            }
            if start_line < line_number {
                analysis.add_to_ignore(start_line..line_number);
            }
            analysis.ignore.remove(&Lines::Line(line_number));
            // Ignore multiple lines of fn decl
            let decl_start = func.sig.fn_token.span().start().line + 1;
            let stmts_start = func.block.span().start().line;
            let lines = decl_start..=stmts_start;
            analysis.add_to_ignore(lines);
        }
    }

    fn visit_trait(&mut self, trait_item: &ItemTrait, ctx: &Context) {
        let check_cover = self.check_attr_list(&trait_item.attrs, ctx);
        if check_cover {
            for item in &trait_item.items {
                if let TraitItem::Fn(ref i) = *item {
                    if self.check_attr_list(&i.attrs, ctx) {
                        let item = i.clone();
                        if let Some(block) = item.default {
                            let item_fn = ItemFn {
                                attrs: item.attrs,
                                // Trait functions inherit visibility from the trait
                                vis: trait_item.vis.clone(),
                                sig: item.sig,
                                block: Box::new(block),
                            };
                            // We visit the function and force cover it
                            self.visit_fn(&item_fn, ctx, true);
                        } else {
                            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                            analysis.ignore_tokens(i);
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
                match *item {
                    ImplItem::Fn(ref i) => {
                        let item = i.clone();
                        let item_fn = ItemFn {
                            attrs: item.attrs,
                            vis: item.vis,
                            sig: item.sig,
                            block: Box::new(item.block),
                        };

                        // If the impl is on a generic, we need to force cover
                        let force_cover = !impl_blk.generics.params.is_empty();

                        self.visit_fn(&item_fn, ctx, force_cover);
                    }
                    ImplItem::Type(_) => {
                        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                        analysis.ignore_span(item.span());
                    }
                    _ => {}
                }
            }
            self.visit_generics(&impl_blk.generics, ctx);
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(impl_blk);
        }
    }
}

fn has_generic_arg<'a>(args: impl Iterator<Item = &'a FnArg>) -> bool {
    for arg in args {
        if let FnArg::Typed(pat) = arg {
            if matches!(*pat.ty, Type::ImplTrait(_)) {
                return true;
            }
        }
    }
    false
}

fn is_sig_generic(sig: &Signature) -> bool {
    !sig.generics.params.is_empty() || has_generic_arg(sig.inputs.iter())
}
