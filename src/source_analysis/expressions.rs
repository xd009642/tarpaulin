use crate::source_analysis::prelude::*;
use std::collections::HashSet;
use syn::{punctuated::Pair, punctuated::Punctuated, spanned::Spanned, token::Comma, *};

impl SourceAnalysis {
    pub(crate) fn process_expr(&mut self, expr: &Expr, ctx: &Context) -> SubResult {
        if ctx.config.branch_coverage {
            let branches = self.get_branch_analysis(ctx.file.to_path_buf());
            branches.register_expr(expr);
        }
        let res = match expr {
            Expr::Macro(m) => self.visit_macro_call(&m.mac, ctx),
            Expr::Struct(s) => self.visit_struct_expr(s, ctx),
            Expr::Unsafe(u) => self.visit_unsafe_block(u, ctx),
            Expr::Call(c) => self.visit_callable(c, ctx),
            Expr::MethodCall(m) => self.visit_methodcall(m, ctx),
            Expr::Match(m) => self.visit_match(m, ctx),
            Expr::Block(b) => self.visit_expr_block(b, ctx),
            Expr::If(i) => self.visit_if(i, ctx),
            Expr::While(w) => self.visit_while(w, ctx),
            Expr::ForLoop(f) => self.visit_for(f, ctx),
            Expr::Loop(l) => self.visit_loop(l, ctx),
            Expr::Return(r) => self.visit_return(r, ctx),
            Expr::Closure(c) => self.visit_closure(c, ctx),
            Expr::Path(p) => self.visit_path(p, ctx),
            Expr::Let(l) => self.visit_let(l, ctx),
            Expr::Group(g) => self.process_expr(&g.expr, ctx),
            Expr::Await(a) => self.process_expr(&a.base, ctx),
            Expr::Async(a) => self.visit_block(&a.block, ctx),
            Expr::Try(t) => {
                self.process_expr(&t.expr, ctx);
                SubResult::Definite
            }
            Expr::TryBlock(t) => {
                self.visit_block(&t.block, ctx);
                SubResult::Definite
            }
            // don't try to compute unreachability on other things
            _ => SubResult::Ok,
        };
        if res.is_unreachable() {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(expr);
        }
        res
    }

    fn visit_let(&mut self, let_expr: &ExprLet, ctx: &Context) -> SubResult {
        let check_cover = self.check_attr_list(&let_expr.attrs, ctx);
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        let mut res = SubResult::Ok;
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
                res += self.process_expr(&let_expr.expr, ctx);
            }
        } else {
            analysis.ignore_tokens(let_expr);
        }
        res
    }

    fn visit_path(&mut self, path: &ExprPath, ctx: &Context) -> SubResult {
        if let Some(PathSegment {
            ref ident,
            arguments: _,
        }) = path.path.segments.last()
        {
            if ident == "unreachable_unchecked" {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(path);
                return SubResult::Unreachable;
            }
        }
        SubResult::Ok
    }

    fn visit_return(&mut self, ret: &ExprReturn, ctx: &Context) -> SubResult {
        let check_cover = self.check_attr_list(&ret.attrs, ctx);
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        if check_cover {
            for a in &ret.attrs {
                analysis.ignore_tokens(a);
            }
        } else {
            analysis.ignore_tokens(ret);
        }
        SubResult::Definite
    }

    fn visit_expr_block(&mut self, block: &ExprBlock, ctx: &Context) -> SubResult {
        if self.check_attr_list(&block.attrs, ctx) {
            self.visit_block(&block.block, ctx)
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(block);
            SubResult::Ok
        }
    }

    fn visit_block(&mut self, block: &Block, ctx: &Context) -> SubResult {
        let reachable = self.process_statements(&block.stmts, ctx);
        if reachable.is_unreachable() {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(block);
        }
        reachable
    }

    fn visit_closure(&mut self, closure: &ExprClosure, ctx: &Context) -> SubResult {
        let res = self.process_expr(&closure.body, ctx);
        // Even if a closure is "unreachable" it might be part of a chained method
        // call and I don't want that propagating up.
        if res.is_unreachable() {
            SubResult::Ok
        } else {
            res
        }
    }

    fn visit_match(&mut self, mat: &ExprMatch, ctx: &Context) -> SubResult {
        // a match with some arms is unreachable iff all its arms are unreachable
        let mut result = None;
        for arm in &mat.arms {
            if self.check_attr_list(&arm.attrs, ctx) {
                let reachable = self.process_expr(&arm.body, ctx);
                if reachable.is_reachable() {
                    let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                    let span = arm.pat.span();
                    for line in span.start().line..span.end().line {
                        analysis.logical_lines.insert(line + 1, span.start().line);
                    }
                    result = result.map(|x| x + reachable).or(Some(reachable));
                }
            } else {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(arm);
            }
        }
        if let Some(result) = result {
            result
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(mat);
            SubResult::Unreachable
        }
    }

    fn visit_if(&mut self, if_block: &ExprIf, ctx: &Context) -> SubResult {
        // an if expression is unreachable iff both its branches are unreachable

        let mut reachable = self.process_expr(&if_block.cond, ctx);
        reachable += self.visit_block(&if_block.then_branch, ctx);
        if let Some((_, ref else_block)) = if_block.else_branch {
            reachable += self.process_expr(else_block, ctx);
        } else {
            // an empty else branch is reachable
            reachable += SubResult::Ok;
        }
        if reachable.is_unreachable() {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(if_block);
            SubResult::Unreachable
        } else {
            reachable
        }
    }

    fn visit_while(&mut self, whl: &ExprWhile, ctx: &Context) -> SubResult {
        if self.check_attr_list(&whl.attrs, ctx) {
            // a while block is unreachable iff its body is
            if self.visit_block(&whl.body, ctx).is_unreachable() {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(whl);
                SubResult::Unreachable
            } else {
                SubResult::Definite
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(whl);
            SubResult::Definite
        }
    }

    fn visit_for(&mut self, for_loop: &ExprForLoop, ctx: &Context) -> SubResult {
        if self.check_attr_list(&for_loop.attrs, ctx) {
            // a for block is unreachable iff its body is
            if self.visit_block(&for_loop.body, ctx).is_unreachable() {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(for_loop);
                SubResult::Unreachable
            } else {
                SubResult::Definite
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(for_loop);
            SubResult::Definite
        }
    }

    fn visit_loop(&mut self, loopex: &ExprLoop, ctx: &Context) -> SubResult {
        if self.check_attr_list(&loopex.attrs, ctx) {
            // a loop block is unreachable iff its body is
            // given we can't reason if a loop terminates we should make it as definite as
            // it may last forever
            if self.visit_block(&loopex.body, ctx).is_unreachable() {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(loopex);
                SubResult::Unreachable
            } else {
                SubResult::Definite
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(loopex);
            SubResult::Definite
        }
    }

    fn visit_callable(&mut self, call: &ExprCall, ctx: &Context) -> SubResult {
        if self.check_attr_list(&call.attrs, ctx) {
            if !call.args.is_empty() && call.span().start().line != call.span().end().line {
                let lines = get_coverable_args(&call.args);
                let lines = get_line_range(call).filter(|x| !lines.contains(x));
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.add_to_ignore(lines);
            }
            self.process_expr(&call.func, ctx);
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(call);
        }
        // We can't guess if a callable would actually be unreachable
        SubResult::Ok
    }

    fn visit_methodcall(&mut self, meth: &ExprMethodCall, ctx: &Context) -> SubResult {
        if self.check_attr_list(&meth.attrs, ctx) {
            self.process_expr(&meth.receiver, ctx);
            let start = meth.receiver.span().end().line + 1;
            let range = get_line_range(meth);
            let lines = get_coverable_args(&meth.args);
            let lines = (start..range.end).filter(|x| !lines.contains(x));
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.add_to_ignore(lines);
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(meth);
        }
        // We can't guess if a method would actually be unreachable
        SubResult::Ok
    }

    fn visit_unsafe_block(&mut self, unsafe_expr: &ExprUnsafe, ctx: &Context) -> SubResult {
        let u_line = unsafe_expr.unsafe_token.span().start().line;
        let mut res = SubResult::Ok;
        let blk = &unsafe_expr.block;
        if u_line != blk.brace_token.span.join().start().line || blk.stmts.is_empty() {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(unsafe_expr.unsafe_token);
        } else if let Some(first_stmt) = blk.stmts.get(0) {
            let s = match first_stmt {
                Stmt::Local(l) => l.span(),
                Stmt::Item(i) => i.span(),
                Stmt::Expr(e, _) => e.span(),
            };
            if u_line != s.start().line {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(unsafe_expr.unsafe_token);
            }
            let reachable = self.process_statements(&blk.stmts, ctx);
            if reachable.is_unreachable() {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(unsafe_expr);
                return SubResult::Unreachable;
            }
            res += reachable;
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(unsafe_expr.unsafe_token);
            analysis.ignore_span(blk.span());
        }
        res
    }

    fn visit_struct_expr(&mut self, structure: &ExprStruct, ctx: &Context) -> SubResult {
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
        let x = get_line_range(structure).filter(|x| !cover.contains(x));
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        analysis.add_to_ignore(x);
        // struct expressions are never unreachable by themselves
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
