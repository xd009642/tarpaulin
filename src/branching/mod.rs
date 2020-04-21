use std::collections::HashMap;

/// The start and end of contiguous range of lines. The range is contained within
/// `start..end`
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LineRange {
    /// Start of the line range (inclusive)
    start: usize,
    /// End of the line range (exclusive)
    end: usize,
}

impl LineRange {
    /// Returns true if the line is contained within the line range
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && line < self.end
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

/// Coverage context for all the branches
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BranchAnalysis {
    /// Each key is `LineRange` showing a region of the code containing a set of branches with the
    /// value being a `LineRange` for each branch in the code
    /// TODO consider BTreeMap then can order on line range start
    branches: HashMap<LineRange, Branches>,
}

impl BranchAnalysis {
    /// Returns true if the line is part of a branch
    pub fn is_branch(&self, line: usize) -> bool {
        self.branches.iter().any(|(k, _)| k.contains(line))
    }
}
