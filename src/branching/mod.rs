use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BranchContext {
    /// Map of files and the branches contained within them
    files: HashMap<PathBuf, BranchAnalysis>,
}

impl BranchContext {
    pub fn is_branch<P: AsRef<Path>>(&self, path: P, line: usize) -> bool {
        if let Some(file) = self.files.get(path.as_ref()) {
            file.is_branch(line)
        } else {
            false
        }
    }
}

/// Coverage context for all the branches
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BranchAnalysis {
    /// Each key is `LineRange` showing a region of the code containing a set of branches with the
    /// value being a `LineRange` for each branch in the code
    branches: BTreeMap<LineRange, Branches>,
}

impl BranchAnalysis {
    /// Returns true if the line is part of a branch
    pub fn is_branch(&self, line: usize) -> bool {
        self.branches.iter().any(|(k, _)| k.contains(line))
    }

    pub fn register_expr(&mut self, expr: &Expr) {
        let range = LineRange::from(expr);
        match *expr {
            Expr::If(ref e) => {
                self.branches.insert(range, e.into());
            }
            Expr::Match(ref e) => {
                self.branches.insert(range, e.into());
            }
            Expr::ForLoop(ref e) => {
                self.branches.insert(range, e.into());
            }
            Expr::While(ref e) => {
                self.branches.insert(range, e.into());
            }
            _ => {}
        }
    }
}

/// Represents possible branches through an execution
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Branches {
    /// Line ranges for each branch
    ranges: Vec<LineRange>,
    /// Whether there is an implicit or empty default branch i.e. missing or empty `else` in an
    /// `if` statement
    implicit_default: bool,
}

impl From<&ExprIf> for Branches {
    fn from(expr: &ExprIf) -> Self {
        let mut then = LineRange::from(&expr.then_branch);
        then.end -= 1;
        let mut ranges = vec![then];
        let mut else_block = expr.else_branch.as_ref().map(|x| x.1.clone());
        let mut implicit_default = else_block.is_none();
        while let Some(el) = else_block {
            let mut lr = LineRange::from(&el);
            if let Expr::If(ref i) = *el {
                then = LineRange::from(&i.then_branch);
                then.end -= 1;
                ranges.push(then);
                else_block = i.else_branch.as_ref().map(|x| x.1.clone());
                implicit_default = else_block.is_none();
                if let Some(s) = &else_block {
                    let lr2 = LineRange::from(s);
                    if lr2.start < lr.end {
                        lr.end = lr2.start - 1;
                    }
                    ranges.push(lr);
                }
            } else {
                else_block = None;
                ranges.push(lr);
            }
        }
        Self {
            ranges,
            implicit_default,
        }
    }
}

impl From<&ExprMatch> for Branches {
    fn from(expr: &ExprMatch) -> Self {
        let ranges = expr
            .arms
            .iter()
            .map(|x| LineRange::from(x))
            .collect::<Vec<_>>();
        Self {
            implicit_default: false,
            ranges,
        }
    }
}

impl From<&ExprForLoop> for Branches {
    fn from(expr: &ExprForLoop) -> Self {
        let mut range = LineRange::from(&expr.body);
        range.start = expr.expr.span().end().line + 1;
        Self {
            implicit_default: true,
            ranges: vec![range],
        }
    }
}

impl From<&ExprWhile> for Branches {
    fn from(expr: &ExprWhile) -> Self {
        let mut range = LineRange::from(&expr.body);
        range.start = expr.cond.span().end().line + 1;
        Self {
            implicit_default: true,
            ranges: vec![range],
        }
    }
}

/// The start and end of contiguous range of lines. The range is contained within
/// `start..end`
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct LineRange {
    /// Start of the line range (inclusive)
    start: usize,
    /// End of the line range (exclusive)
    end: usize,
}

impl<T> From<T> for LineRange
where
    T: spanned::Spanned,
{
    fn from(t: T) -> Self {
        Self {
            start: t.span().start().line,
            end: t.span().end().line + 1,
        }
    }
}

impl LineRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns true if the line is contained within the line range
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && line < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{parse_file, Expr, Item, Stmt};

    #[test]
    fn if_branches() {
        let source = "fn foo(x: i32) {
            if x > 0 {
                println!(\"BOO\");
            } else if x < 0 {
                todo!()
            }
        }";
        let parser = parse_file(source).unwrap();
        let func = &parser.items[0];
        let mut branches = None;
        if let Item::Fn(func) = func {
            for stmt in &func.block.stmts {
                match stmt {
                    Stmt::Expr(e) => {
                        if let Expr::If(i) = e {
                            branches = Some(Branches::from(i));
                        }
                    }
                    Stmt::Semi(e, _) => {
                        if let Expr::If(i) = e {
                            branches = Some(Branches::from(i));
                        }
                    }
                    _ => {}
                }
            }
        }
        let branches = branches.unwrap();
        assert!(branches.implicit_default);
        assert_eq!(branches.ranges.len(), 2);
        assert_eq!(branches.ranges[0], LineRange::new(2, 4));
        assert_eq!(branches.ranges[1], LineRange::new(4, 6));
        let source = "fn foo(x: i32) {
            if x > 0 {
                println!(\"BOO\");
            } else {
                todo!()
            }
        }";
        let parser = parse_file(source).unwrap();
        let func = &parser.items[0];
        let mut branches = None;
        if let Item::Fn(func) = func {
            for stmt in &func.block.stmts {
                match stmt {
                    Stmt::Expr(e) => {
                        if let Expr::If(i) = e {
                            branches = Some(Branches::from(i));
                        }
                    }
                    Stmt::Semi(e, _) => {
                        if let Expr::If(i) = e {
                            branches = Some(Branches::from(i));
                        }
                    }
                    _ => {}
                }
            }
        }
        let branches = branches.unwrap();
        assert!(!branches.implicit_default);
        assert_eq!(branches.ranges.len(), 2);
        assert_eq!(branches.ranges[0], LineRange::new(2, 4));
        assert_eq!(branches.ranges[1], LineRange::new(4, 7));
    }

    #[test]
    fn match_branches() {
        let source = "fn foo(x: i32) {
            match 4 {
                0 => {
                    todo!();
                },
                1 => {
                    todo!();
                },
                _ => {},
            }
        }";
        let parser = parse_file(source).unwrap();
        let func = &parser.items[0];
        let mut branches = None;
        if let Item::Fn(func) = func {
            for stmt in &func.block.stmts {
                match stmt {
                    Stmt::Expr(e) => {
                        if let Expr::Match(m) = e {
                            branches = Some(Branches::from(m));
                        }
                    }
                    Stmt::Semi(e, _) => {
                        if let Expr::Match(m) = e {
                            branches = Some(Branches::from(m));
                        }
                    }
                    _ => {}
                }
            }
        }
        let branches = branches.unwrap();
        assert!(!branches.implicit_default);
        assert_eq!(branches.ranges.len(), 3);
        assert_eq!(branches.ranges[0], LineRange::new(3, 6));
        assert_eq!(branches.ranges[1], LineRange::new(6, 9));
        assert_eq!(branches.ranges[2], LineRange::new(9, 10));
    }

    #[test]
    fn for_branches() {
        let source = "fn foo(x: i32) {
            for i in 0..1 {
                todo!();
            }
        }";
        let parser = parse_file(source).unwrap();
        let func = &parser.items[0];
        let mut branches = None;
        if let Item::Fn(func) = func {
            for stmt in &func.block.stmts {
                match stmt {
                    Stmt::Expr(e) => {
                        if let Expr::ForLoop(f) = e {
                            branches = Some(Branches::from(f));
                        }
                    }
                    Stmt::Semi(e, _) => {
                        if let Expr::ForLoop(f) = e {
                            branches = Some(Branches::from(f));
                        }
                    }
                    _ => {}
                }
            }
        }
        let branches = branches.unwrap();
        assert!(branches.implicit_default);
        assert_eq!(branches.ranges.len(), 1);
        assert_eq!(branches.ranges[0], LineRange::new(3, 5));
    }

    #[test]
    fn while_branches() {
        let source = "fn foo(x: i32) {
            let mut i = 0;
            while i < 10 {
                i+=1;
            }
        }";
        let parser = parse_file(source).unwrap();
        let func = &parser.items[0];
        let mut branches = None;
        if let Item::Fn(func) = func {
            for stmt in &func.block.stmts {
                match stmt {
                    Stmt::Expr(e) => {
                        if let Expr::While(w) = e {
                            branches = Some(Branches::from(w));
                        }
                    }
                    Stmt::Semi(e, _) => {
                        if let Expr::While(w) = e {
                            branches = Some(Branches::from(w));
                        }
                    }
                    _ => {}
                }
            }
        }
        let branches = branches.unwrap();
        assert!(branches.implicit_default);
        assert_eq!(branches.ranges.len(), 1);
        assert_eq!(branches.ranges[0], LineRange::new(4, 6));
    }
}
