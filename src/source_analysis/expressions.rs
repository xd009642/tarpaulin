use crate::source_analysis::prelude::*;
use std::collections::HashSet;
use syn::{punctuated::Pair, punctuated::Punctuated, spanned::Spanned, token::Comma, *};

pub(crate) fn process_expr(expr: &Expr, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    let res = match *expr {
        Expr::Macro(ref m) => visit_macro_call(&m.mac, ctx, analysis),
        Expr::Struct(ref s) => visit_struct_expr(&s, analysis),
        Expr::Unsafe(ref u) => visit_unsafe_block(&u, ctx, analysis),
        Expr::Call(ref c) => visit_callable(&c, ctx, analysis),
        Expr::MethodCall(ref m) => visit_methodcall(&m, ctx, analysis),
        Expr::Match(ref m) => visit_match(&m, ctx, analysis),
        Expr::Block(ref b) => visit_expr_block(&b, ctx, analysis),
        Expr::If(ref i) => visit_if(&i, ctx, analysis),
        Expr::While(ref w) => visit_while(&w, ctx, analysis),
        Expr::ForLoop(ref f) => visit_for(&f, ctx, analysis),
        Expr::Loop(ref l) => visit_loop(&l, ctx, analysis),
        Expr::Return(ref r) => visit_return(&r, ctx, analysis),
        Expr::Closure(ref c) => visit_closure(&c, ctx, analysis),
        Expr::Path(ref p) => visit_path(&p, analysis),
        Expr::Let(ref l) => visit_let(&l, ctx, analysis),
        // don't try to compute unreachability on other things
        _ => SubResult::Ok,
    };
    if let SubResult::Unreachable = res {
        analysis.ignore_tokens(expr);
    }
    res
}

fn visit_let(let_expr: &ExprLet, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    let check_cover = check_attr_list(&let_expr.attrs, ctx, analysis);
    if check_cover {
        for a in &let_expr.attrs {
            analysis.ignore_tokens(a);
        }
        let spn = let_expr.span();
        let base_line = let_expr.let_token.span().start().line;
        if base_line != spn.end().line {
            // Now check the other lines
            let lhs = let_expr.pat.span();
            if lhs.start().line != base_line {
                analysis.logical_lines.insert(lhs.start().line, base_line);
            }
            let eq = let_expr.eq_token.span();
            if eq.start().line != base_line {
                analysis.logical_lines.insert(eq.start().line, base_line);
            }
            if let_expr.expr.span().start().line != base_line {
                analysis
                    .logical_lines
                    .insert(let_expr.expr.span().start().line, base_line);
            }
            process_expr(&let_expr.expr, ctx, analysis);
        }
    } else {
        analysis.ignore_tokens(let_expr);
    }
    SubResult::Ok
}

fn visit_path(path: &ExprPath, analysis: &mut LineAnalysis) -> SubResult {
    if let Some(PathSegment {
        ref ident,
        arguments: _,
    }) = path.path.segments.last()
    {
        if ident == "unreachable_unchecked" {
            analysis.ignore_tokens(path);
            return SubResult::Unreachable;
        }
    }
    SubResult::Ok
}

fn visit_return(ret: &ExprReturn, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    let check_cover = check_attr_list(&ret.attrs, ctx, analysis);
    if check_cover {
        for a in &ret.attrs {
            analysis.ignore_tokens(a);
        }
    } else {
        analysis.ignore_tokens(ret);
    }
    SubResult::Ok
}

fn visit_expr_block(block: &ExprBlock, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    if check_attr_list(&block.attrs, ctx, analysis) {
        visit_block(&block.block, ctx, analysis)
    } else {
        analysis.ignore_tokens(block);
        SubResult::Ok
    }
}

fn visit_block(block: &Block, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    if let SubResult::Unreachable = process_statements(&block.stmts, ctx, analysis) {
        analysis.ignore_tokens(block);
        SubResult::Unreachable
    } else {
        SubResult::Ok
    }
}

fn visit_closure(closure: &ExprClosure, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    process_expr(&closure.body, ctx, analysis);
    // Even if a closure is "unreachable" it might be part of a chained method
    // call and I don't want that propagating up.
    SubResult::Ok
}

fn visit_match(mat: &ExprMatch, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    // a match with some arms is unreachable iff all its arms are unreachable
    let mut reachable_arm = false;
    for arm in &mat.arms {
        if check_attr_list(&arm.attrs, ctx, analysis) {
            if let SubResult::Ok = process_expr(&arm.body, ctx, analysis) {
                reachable_arm = true
            }
        } else {
            analysis.ignore_tokens(arm);
        }
    }
    if !reachable_arm {
        analysis.ignore_tokens(mat);
        SubResult::Unreachable
    } else {
        SubResult::Ok
    }
}

fn visit_if(if_block: &ExprIf, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    // an if expression is unreachable iff both its branches are unreachable
    let mut reachable_arm = false;

    process_expr(&if_block.cond, ctx, analysis);

    if let SubResult::Ok = visit_block(&if_block.then_branch, ctx, analysis) {
        reachable_arm = true;
    }
    if let Some((_, ref else_block)) = if_block.else_branch {
        if let SubResult::Ok = process_expr(&else_block, ctx, analysis) {
            reachable_arm = true;
        }
    } else {
        // an empty else branch is reachable
        reachable_arm = true;
    }
    if !reachable_arm {
        analysis.ignore_tokens(if_block);
        SubResult::Unreachable
    } else {
        SubResult::Ok
    }
}

fn visit_while(whl: &ExprWhile, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    if check_attr_list(&whl.attrs, ctx, analysis) {
        // a while block is unreachable iff its body is
        if let SubResult::Unreachable = visit_block(&whl.body, ctx, analysis) {
            analysis.ignore_tokens(whl);
            SubResult::Unreachable
        } else {
            SubResult::Ok
        }
    } else {
        analysis.ignore_tokens(whl);
        SubResult::Ok
    }
}

fn visit_for(for_loop: &ExprForLoop, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    if check_attr_list(&for_loop.attrs, ctx, analysis) {
        // a for block is unreachable iff its body is
        if let SubResult::Unreachable = visit_block(&for_loop.body, ctx, analysis) {
            analysis.ignore_tokens(for_loop);
            SubResult::Unreachable
        } else {
            SubResult::Ok
        }
    } else {
        analysis.ignore_tokens(for_loop);
        SubResult::Ok
    }
}

fn visit_loop(loopex: &ExprLoop, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    if check_attr_list(&loopex.attrs, ctx, analysis) {
        // a loop block is unreachable iff its body is
        if let SubResult::Unreachable = visit_block(&loopex.body, ctx, analysis) {
            analysis.ignore_tokens(loopex);
            SubResult::Unreachable
        } else {
            SubResult::Ok
        }
    } else {
        analysis.ignore_tokens(loopex);
        SubResult::Ok
    }
}

fn get_coverable_args(args: &Punctuated<Expr, Comma>) -> HashSet<usize> {
    let mut lines: HashSet<usize> = HashSet::new();
    for a in args.iter() {
        match *a {
            Expr::Lit(_) => {}
            _ => {
                for i in get_line_range(a) {
                    lines.insert(i);
                }
            }
        }
    }
    lines
}

fn visit_callable(call: &ExprCall, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    if check_attr_list(&call.attrs, ctx, analysis) {
        if !call.args.is_empty() {
            let lines = get_coverable_args(&call.args);
            let lines = get_line_range(call)
                .filter(|x| !lines.contains(&x))
                .collect::<Vec<_>>();
            analysis.add_to_ignore(&lines);
        }
        process_expr(&call.func, ctx, analysis);
    } else {
        analysis.ignore_tokens(call);
    }
    // We can't guess if a callable would actually be unreachable
    SubResult::Ok
}

fn visit_methodcall(
    meth: &ExprMethodCall,
    ctx: &Context,
    analysis: &mut LineAnalysis,
) -> SubResult {
    if check_attr_list(&meth.attrs, ctx, analysis) {
        process_expr(&meth.receiver, ctx, analysis);
        let start = meth.receiver.span().end().line + 1;
        let range = get_line_range(meth);
        let lines = get_coverable_args(&meth.args);
        let lines = (start..range.end)
            .filter(|x| !lines.contains(&x))
            .collect::<Vec<_>>();
        analysis.add_to_ignore(&lines);
    } else {
        analysis.ignore_tokens(meth);
    }
    // We can't guess if a method would actually be unreachable
    SubResult::Ok
}

fn visit_unsafe_block(
    unsafe_expr: &ExprUnsafe,
    ctx: &Context,
    analysis: &mut LineAnalysis,
) -> SubResult {
    let u_line = unsafe_expr.unsafe_token.span().start().line;

    let blk = &unsafe_expr.block;
    if u_line != blk.brace_token.span.start().line || blk.stmts.is_empty() {
        analysis.ignore_tokens(unsafe_expr.unsafe_token);
    } else if let Some(ref first_stmt) = blk.stmts.get(0) {
        let s = match **first_stmt {
            Stmt::Local(ref l) => l.span(),
            Stmt::Item(ref i) => i.span(),
            Stmt::Expr(ref e) => e.span(),
            Stmt::Semi(ref e, _) => e.span(),
        };
        if u_line != s.start().line {
            analysis.ignore_tokens(unsafe_expr.unsafe_token);
        }
        if let SubResult::Unreachable = process_statements(&blk.stmts, ctx, analysis) {
            analysis.ignore_tokens(unsafe_expr);
            return SubResult::Unreachable;
        }
    } else {
        analysis.ignore_tokens(unsafe_expr.unsafe_token);
        analysis.ignore_span(blk.brace_token.span);
    }
    SubResult::Ok
}

fn visit_struct_expr(structure: &ExprStruct, analysis: &mut LineAnalysis) -> SubResult {
    let mut cover: HashSet<usize> = HashSet::new();
    for field in structure.fields.pairs() {
        let first = match field {
            Pair::Punctuated(t, _) => t,
            Pair::End(t) => t,
        };
        let span = match first.member {
            Member::Named(ref i) => i.span(),
            Member::Unnamed(ref i) => i.span,
        };
        match first.expr {
            Expr::Lit(_) | Expr::Path(_) => {}
            _ => {
                cover.insert(span.start().line);
            }
        }
    }
    let x = get_line_range(structure)
        .filter(|x| !cover.contains(&x))
        .collect::<Vec<usize>>();
    analysis.add_to_ignore(&x);
    // struct expressions are never unreachable by themselves
    SubResult::Ok
}
