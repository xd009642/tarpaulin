use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use syn::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BranchContext {
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
    /// TODO consider BTreeMap then can order on line range start
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
        let mut ranges = vec![];
        let mut else_block = expr.else_branch.as_ref().map(|x| x.1.clone());
        let mut implicit_default = else_block.is_none();
        while let Some(el) = else_block {
            let mut lr = LineRange::from(&el);
            if let Expr::If(ref i) = *el {
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
        Self {
            implicit_default: true,
            ranges: vec![expr.body.clone().into()],
        }
    }
}

impl From<&ExprWhile> for Branches {
    fn from(expr: &ExprWhile) -> Self {
        Self {
            implicit_default: true,
            ranges: vec![expr.body.clone().into()],
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
    /// Returns true if the line is contained within the line range
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && line < self.end
    }
}
