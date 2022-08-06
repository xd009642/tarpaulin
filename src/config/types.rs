use clap::arg_enum;
use serde::{Deserialize, Serialize};

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum Color {
        Auto,
        Always,
        Never,
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum TraceEngine {
        Auto,
        Ptrace,
        Llvm,
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum Mode {
        Test,
        Build
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum RunType {
        Tests,
        Doctests,
        Benchmarks,
        Examples,
        Lib,
        Bins,
        AllTargets,
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
    #[non_exhaustive]
    pub enum OutputFile {
        Json,
        Stdout,
        Xml,
        Html,
        Lcov,
    }
}

impl Default for OutputFile {
    #[inline]
    fn default() -> Self {
        OutputFile::Stdout
    }
}
