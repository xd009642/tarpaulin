use crate::source_analysis::prelude::*;
use syn::{spanned::Spanned, *};

pub(crate) fn process_statements(
    stmts: &[Stmt],
    ctx: &Context,
    analysis: &mut LineAnalysis,
) -> SubResult {
    // in a list of statements, if any of them is unreachable, the whole list is
    // unreachable
    let mut unreachable = false;
    for stmt in stmts.iter() {
        let res = match *stmt {
            Stmt::Item(ref i) => process_items(&[i.clone()], ctx, analysis),
            Stmt::Expr(ref i) | Stmt::Semi(ref i, _) => process_expr(&i, ctx, analysis),
            Stmt::Local(ref i) => process_local(&i, ctx, analysis),
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

fn process_local(local: &Local, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    if let Some((eq, expr)) = &local.init {
        let check_cover = check_attr_list(&local.attrs, ctx, analysis);
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
                let eq = eq.span();
                if eq.start().line != base_line {
                    analysis.logical_lines.insert(eq.start().line, base_line);
                }
                if expr.span().start().line != base_line {
                    analysis
                        .logical_lines
                        .insert(expr.span().start().line, base_line);
                }
                process_expr(&expr, ctx, analysis);
            }
        } else {
            analysis.ignore_tokens(local);
        }
    }

    SubResult::Ok
}
