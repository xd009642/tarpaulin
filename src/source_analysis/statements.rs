use crate::source_analysis::prelude::*;
use syn::{spanned::Spanned, *};

impl SourceAnalysis {
    pub(crate) fn process_statements(&mut self, stmts: &[Stmt], ctx: &Context) -> SubResult {
        // in a list of statements, if any of them is unreachable, the whole list is
        // unreachable
        let mut unreachable = false;
        for stmt in stmts.iter() {
            let res = match *stmt {
                Stmt::Item(ref i) => self.process_items(&[i.clone()], ctx),
                Stmt::Expr(ref i) | Stmt::Semi(ref i, _) => self.process_expr(&i, ctx),
                Stmt::Local(ref i) => self.process_local(&i, ctx),
            };
            if let SubResult::Unreachable = res {
                unreachable = true;
            }
        }
        // We must be in a block, the parent will handle marking the span as unreachable
        if unreachable {
            SubResult::Unreachable
        } else {
            SubResult::Ok
        }
    }

    fn process_local(&mut self, local: &Local, ctx: &Context) -> SubResult {
        if let Some((eq, expr)) = &local.init {
            let check_cover = self.check_attr_list(&local.attrs, ctx);
            if check_cover {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
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
                    let eq = eq.span();
                    if eq.start().line != base_line {
                        analysis.logical_lines.insert(eq.start().line, base_line);
                    }
                    if expr.span().start().line != base_line {
                        analysis
                            .logical_lines
                            .insert(expr.span().start().line, base_line);
                    }
                    std::mem::drop(analysis);
                    self.process_expr(&expr, ctx);
                }
            } else {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(local);
            }
        }

        SubResult::Ok
    }
}
