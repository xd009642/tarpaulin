use crate::source_analysis::prelude::*;
use syn::*;

impl SourceAnalysis {
    pub(crate) fn process_statements(&mut self, stmts: &[Stmt], ctx: &Context) -> SubResult {
        // in a list of statements, if any of them is unreachable, the whole list is
        // unreachable
        let mut unreachable = false;
        let mut definite = false;
        for stmt in stmts.iter() {
            let res = match stmt {
                Stmt::Item(i) => self.process_items(&[i.clone()], ctx),
                Stmt::Expr(i, _) => self.process_expr(i, ctx),
                Stmt::Local(i) => self.process_local(i, ctx),
                Stmt::Macro(i) => self.process_macro(i, ctx),
            };
            unreachable |= res.is_unreachable();
            if SubResult::Definite == res {
                definite = true;
            }
        }
        // We must be in a block, the parent will handle marking the span as unreachable
        if unreachable && !definite {
            SubResult::Unreachable
        } else if definite {
            SubResult::Definite
        } else {
            SubResult::Ok
        }
    }

    fn process_macro(&mut self, mac: &StmtMacro, ctx: &Context) -> SubResult {
        let check_cover = self.check_attr_list(&mac.attrs, ctx);
        if check_cover {
            if let Some(macro_name) = mac.mac.path.segments.last() {
                let (sub, should_ignore) = ignore_macro_name(&macro_name.ident, ctx);
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                if should_ignore {
                    analysis.ignore_tokens(mac);
                } else {
                    // lets just merge the macros into one big logical line
                    let start = mac.span().start().line;
                    for i in start..mac.span().end().line {
                        analysis.logical_lines.insert(i + 1, start);
                    }
                }
                sub
            } else {
                SubResult::Ok
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(mac);
            SubResult::Ok
        }
    }

    fn process_local(&mut self, local: &Local, ctx: &Context) -> SubResult {
        let mut result = SubResult::Ok;
        if let Some(init) = &local.init {
            // Process if the local wasn't ignored with an attribute
            let check_cover = self.check_attr_list(&local.attrs, ctx);
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());

            if check_cover {
                for a in &local.attrs {
                    analysis.ignore_tokens(a);
                }
                let spn = local.span();
                let base_line = local.let_token.span().start().line;
                if base_line != spn.end().line {
                    // Now check the other lines
                    let lhs = local.pat.span();
                    if lhs.start().line != base_line {
                        analysis.logical_lines.insert(lhs.start().line, base_line);
                    }
                    let eq = init.eq_token.span();
                    if eq.start().line != base_line {
                        analysis.logical_lines.insert(eq.start().line, base_line);
                    }
                    if init.expr.span().start().line != base_line {
                        analysis
                            .logical_lines
                            .insert(init.expr.span().start().line, base_line);
                    }
                    result += self.process_expr(&init.expr, ctx);
                    if let Some((_, expr)) = &init.diverge {
                        self.process_expr(expr, ctx);
                    }
                }
            } else {
                analysis.ignore_tokens(local);
            }
        }
        result
    }
}
