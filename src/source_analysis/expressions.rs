use crate::source_analysis::prelude::*;
use std::collections::HashSet;
use syn::{
    punctuated::{Pair, Punctuated},
    spanned::Spanned,
    token::Comma,
    *,
};

impl SourceAnalysis {
    pub(crate) fn process_expr(&mut self, expr: &Expr, ctx: &Context) -> SubResult {
        if ctx.config.branch_coverage {
            let branches = self.get_branch_analysis(ctx.file.to_path_buf());
            branches.register_expr(expr);
        }
        let res = match *expr {
            Expr::Macro(ref m) => self.visit_macro_call(&m.mac, ctx),
            Expr::Binary(ref b) => self.visit_binary(b, ctx),
            Expr::Assign(ref a) => self.visit_assign(&a, ctx),
            Expr::AssignOp(ref a) => self.visit_assign_op(&a, ctx),
            Expr::Struct(ref s) => self.visit_struct_expr(&s, ctx),
            Expr::Unsafe(ref u) => self.visit_unsafe_block(&u, ctx),
            Expr::Call(ref c) => self.visit_callable(&c, ctx),
            Expr::MethodCall(ref m) => self.visit_methodcall(&m, ctx),
            Expr::Match(ref m) => self.visit_match(&m, ctx),
            Expr::Block(ref b) => self.visit_expr_block(&b, ctx),
            Expr::If(ref i) => self.visit_if(&i, ctx),
            Expr::While(ref w) => self.visit_while(&w, ctx),
            Expr::ForLoop(ref f) => self.visit_for(&f, ctx),
            Expr::Loop(ref l) => self.visit_loop(&l, ctx),
            Expr::Return(ref r) => self.visit_return(&r, ctx),
            Expr::Closure(ref c) => self.visit_closure(&c, ctx),
            Expr::Path(ref p) => self.visit_path(&p, ctx),
            Expr::Let(ref l) => self.visit_let(&l, ctx),
            Expr::Group(ref g) => self.process_expr(&g.expr, ctx),
            Expr::Await(ref a) => self.process_expr(&a.base, ctx),
            Expr::Async(ref a) => self.visit_block(&a.block, ctx),
            Expr::Try(ref t) => self.process_expr(&t.expr, ctx),
            Expr::TryBlock(ref t) => self.visit_block(&t.block, ctx),
            // don't try to compute unreachability on other things
            _ => SubResult::Ok,
        };
        if let SubResult::Unreachable = res {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(expr);
        }
        res
    }

    fn visit_assign(&mut self, assign: &ExprAssign, ctx: &Context) -> SubResult {
        let check_cover = self.check_attr_list(&assign.attrs, ctx);
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        if check_cover {
            self.process_expr(&assign.left, ctx);
            self.process_expr(&assign.right, ctx);
        } else {
            analysis.ignore_tokens(assign);
        }
        SubResult::Ok
    }

    fn visit_assign_op(&mut self, assign: &ExprAssignOp, ctx: &Context) -> SubResult {
        let check_cover = self.check_attr_list(&assign.attrs, ctx);
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        if check_cover {
            self.process_expr(&assign.left, ctx);
            self.process_expr(&assign.right, ctx);
        } else {
            analysis.ignore_tokens(assign);
        }
        SubResult::Ok
    }

    fn visit_binary(&mut self, binary: &ExprBinary, ctx: &Context) -> SubResult {
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        analysis.cover_logical_line(binary.span());
        SubResult::Ok
    }

    fn visit_let(&mut self, let_expr: &ExprLet, ctx: &Context) -> SubResult {
        let check_cover = self.check_attr_list(&let_expr.attrs, ctx);
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        if check_cover {
            for a in &let_expr.attrs {
                analysis.ignore_tokens(a);
            }
            let spn = let_expr.span();
            let base_line = let_expr.let_token.span().start().line;
            analysis.cover_span(let_expr.let_token.span(), None);
            if base_line != spn.end().line {
                // Now check the other lines
                let lhs = let_expr.pat.span();
                analysis.cover.insert(base_line);
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
                self.process_expr(&let_expr.expr, ctx);
            }
        } else {
            analysis.ignore_tokens(let_expr);
        }
        SubResult::Ok
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
        SubResult::Ok
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
        if let SubResult::Unreachable = self.process_statements(&block.stmts, ctx) {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(block);
            SubResult::Unreachable
        } else {
            SubResult::Ok
        }
    }

    fn visit_closure(&mut self, closure: &ExprClosure, ctx: &Context) -> SubResult {
        self.process_expr(&closure.body, ctx);
        // Even if a closure is "unreachable" it might be part of a chained method
        // call and I don't want that propagating up.
        SubResult::Ok
    }

    fn visit_match(&mut self, mat: &ExprMatch, ctx: &Context) -> SubResult {
        // a match with some arms is unreachable iff all its arms are unreachable
        let mut reachable_arm = false;
        for arm in &mat.arms {
            if self.check_attr_list(&arm.attrs, ctx) {
                if let SubResult::Ok = self.process_expr(&arm.body, ctx) {
                    reachable_arm = true
                }
            } else {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(arm);
            }
        }
        if !reachable_arm {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(mat);
            SubResult::Unreachable
        } else {
            SubResult::Ok
        }
    }

    fn visit_if(&mut self, if_block: &ExprIf, ctx: &Context) -> SubResult {
        // an if expression is unreachable iff both its branches are unreachable
        let mut reachable_arm = false;

        self.process_expr(&if_block.cond, ctx);

        if let SubResult::Ok = self.visit_block(&if_block.then_branch, ctx) {
            reachable_arm = true;
        }
        if let Some((_, ref else_block)) = if_block.else_branch {
            if let SubResult::Ok = self.process_expr(&else_block, ctx) {
                reachable_arm = true;
            }
        } else {
            // an empty else branch is reachable
            reachable_arm = true;
        }
        if !reachable_arm {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(if_block);
            SubResult::Unreachable
        } else {
            SubResult::Ok
        }
    }

    fn visit_while(&mut self, whl: &ExprWhile, ctx: &Context) -> SubResult {
        if self.check_attr_list(&whl.attrs, ctx) {
            // a while block is unreachable iff its body is
            if let SubResult::Unreachable = self.visit_block(&whl.body, ctx) {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(whl);
                SubResult::Unreachable
            } else {
                self.process_expr(&whl.cond, ctx)
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(whl);
            SubResult::Ok
        }
    }

    fn visit_for(&mut self, for_loop: &ExprForLoop, ctx: &Context) -> SubResult {
        if self.check_attr_list(&for_loop.attrs, ctx) {
            // a for block is unreachable iff its body is
            if let SubResult::Unreachable = self.visit_block(&for_loop.body, ctx) {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(for_loop);
                SubResult::Unreachable
            } else {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.cover_span(for_loop.pat.span(), None);
                self.process_expr(&for_loop.expr, ctx);
                SubResult::Ok
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(for_loop);
            SubResult::Ok
        }
    }

    fn visit_loop(&mut self, loopex: &ExprLoop, ctx: &Context) -> SubResult {
        if self.check_attr_list(&loopex.attrs, ctx) {
            // a loop block is unreachable iff its body is
            if let SubResult::Unreachable = self.visit_block(&loopex.body, ctx) {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(loopex);
                SubResult::Unreachable
            } else {
                SubResult::Ok
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(loopex);
            SubResult::Ok
        }
    }

    fn visit_callable(&mut self, call: &ExprCall, ctx: &Context) -> SubResult {
        if self.check_attr_list(&call.attrs, ctx) {
            self.process_expr(&call.func, ctx);
            self.visit_args(call.paren_token.span, &call.args, ctx);
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
            self.handle_method_chains(meth, ctx);
            if let Some(spn) = meth.method.span().join(meth.paren_token.span) {
                self.visit_args(spn, &meth.args, ctx);
            } else {
                self.visit_args(meth.paren_token.span, &meth.args, ctx);
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(meth);
        }
        // We can't guess if a method would actually be unreachable
        SubResult::Ok
    }

    fn handle_method_chains(&mut self, meth: &ExprMethodCall, ctx: &Context) {
        let mut spans = vec![meth.method.span()];
        let mut above = meth.receiver.clone();
        while let Expr::MethodCall(meth) = *above {
            spans.push(meth.method.span());
            above = meth.receiver.clone();
            if let Expr::Try(t) = *above {
                above = t.expr.clone();
            }
        }
        spans.push(above.span());
        println!("{:?}", spans);
        if let Some(base) = spans.pop() {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            let base = base.start().line;
            println!("Base line {}", base);
            for span in &spans {
                analysis.cover_logical_line_with_base(*span, base);
            }
            println!("Analysis {:?}", analysis);
        }
    }

    fn visit_args(&mut self, outer_span: Span, args: &Punctuated<Expr, Comma>, ctx: &Context) {
        let base = outer_span.start().line;
        for arg in args.iter() {
            self.process_expr(arg, ctx);
            if arg.span().start().line == arg.span().end().line {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.cover_logical_line_with_base(arg.span(), base);
            } else {
                match arg {
                    Expr::Async(_)
                    | Expr::Await(_)
                    | Expr::Block(_)
                    | Expr::Call(_)
                    | Expr::Closure(_)
                    | Expr::ForLoop(_)
                    | Expr::Group(_)
                    | Expr::If(_)
                    | Expr::Index(_)
                    | Expr::Loop(_)
                    | Expr::Macro(_)
                    | Expr::Match(_)
                    | Expr::MethodCall(_)
                    | Expr::Try(_)
                    | Expr::TryBlock(_)
                    | Expr::Unsafe(_)
                    | Expr::While(_) => {}
                    Expr::Tuple(t) => self.visit_args(outer_span, &t.elems, ctx),
                    e @ _ => {
                        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                        analysis.cover_logical_line_with_base(e.span(), base);
                    }
                }
            }
        }
    }

    fn visit_unsafe_block(&mut self, unsafe_expr: &ExprUnsafe, ctx: &Context) -> SubResult {
        let u_line = unsafe_expr.unsafe_token.span().start().line;

        let blk = &unsafe_expr.block;
        if u_line != blk.brace_token.span.start().line || blk.stmts.is_empty() {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(unsafe_expr.unsafe_token);
        } else if let Some(ref first_stmt) = blk.stmts.get(0) {
            let s = match **first_stmt {
                Stmt::Local(ref l) => l.span(),
                Stmt::Item(ref i) => i.span(),
                Stmt::Expr(ref e) => e.span(),
                Stmt::Semi(ref e, _) => e.span(),
            };
            if u_line != s.start().line {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(unsafe_expr.unsafe_token);
            }
            if let SubResult::Unreachable = self.process_statements(&blk.stmts, ctx) {
                let analysis = self.get_line_analysis(ctx.file.to_path_buf());
                analysis.ignore_tokens(unsafe_expr);
                return SubResult::Unreachable;
            }
        } else {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(unsafe_expr.unsafe_token);
            analysis.ignore_span(blk.brace_token.span);
        }
        SubResult::Ok
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
        let x = get_line_range(structure)
            .filter(|x| !cover.contains(&x))
            .collect::<Vec<usize>>();
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        let cover_vec = cover.iter().copied().collect::<Vec<_>>();
        analysis.add_to_cover(&cover_vec);
        analysis.add_to_ignore(&x);
        // struct expressions are never unreachable by themselves
        SubResult::Ok
    }
}
