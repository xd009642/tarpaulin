use crate::branching::BranchAnalysis;
use crate::config::{Config, RunType};
use crate::path_utils::{get_source_walker, is_source_file};
use lazy_static::lazy_static;
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::*;
use tracing::{trace, warn};
use walkdir::WalkDir;

mod attributes;
mod expressions;
mod items;
mod macros;
mod statements;
#[cfg(test)]
mod tests;

pub(crate) mod prelude {
    pub(crate) use super::*;
    pub(crate) use attributes::*;
    pub(crate) use macros::*;
}

/// Enumeration representing which lines to ignore
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Lines {
    /// Ignore all lines in the file
    All,
    /// A single line to ignore in the file
    Line(usize),
}

/// Represents the results of analysis of a single file. Does not store the file
/// in question as this is expected to be maintained by the user.
#[derive(Clone, Debug, Default)]
pub struct LineAnalysis {
    /// This represents lines that should be ignored in coverage
    /// but may be identifed as coverable in the DWARF tables
    pub ignore: HashSet<Lines>,
    /// This represents lines that should be included in coverage
    /// But may be ignored. Doesn't make sense to cover ALL the lines so this
    /// is just an index.
    pub cover: HashSet<usize>,
    /// Some logical lines may be split between physical lines this shows the
    /// mapping from physical line to logical line to prevent false positives
    /// from expressions split across physical lines
    pub logical_lines: HashMap<usize, usize>,
}

/// When the `LineAnalysis` results are mapped to their files there needs to be
/// an easy way to get the information back. For the container used implement
/// this trait
pub trait SourceAnalysisQuery {
    /// Returns true if the line in the given file should be ignored
    fn should_ignore(&self, path: &Path, l: &usize) -> bool;
    /// Takes a path and line number and normalises it to the logical line
    /// that should be represented in the statistics
    fn normalise(&self, path: &Path, l: usize) -> (PathBuf, usize);
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum SubResult {
    Ok,
    Unreachable,
}

impl SourceAnalysisQuery for HashMap<PathBuf, LineAnalysis> {
    fn should_ignore(&self, path: &Path, l: &usize) -> bool {
        if self.contains_key(path) {
            self.get(path).unwrap().should_ignore(*l)
        } else {
            false
        }
    }

    fn normalise(&self, path: &Path, l: usize) -> (PathBuf, usize) {
        let pb = path.to_path_buf();
        match self.get(path) {
            Some(s) => match s.logical_lines.get(&l) {
                Some(o) => (pb, *o),
                _ => (pb, l),
            },
            _ => (pb, l),
        }
    }
}

impl LineAnalysis {
    /// Creates a new LineAnalysis object
    fn new() -> LineAnalysis {
        Default::default()
    }

    /// Ignore all lines in the file
    pub fn ignore_all(&mut self) {
        self.ignore.clear();
        self.cover.clear();
        self.ignore.insert(Lines::All);
    }

    /// Ignore all tokens in the given token stream
    pub fn ignore_tokens<T>(&mut self, tokens: T)
    where
        T: ToTokens,
    {
        for token in tokens.into_token_stream() {
            self.ignore_span(token.span());
        }
    }

    /// Adds the lines of the provided span to the ignore set
    pub fn ignore_span(&mut self, span: Span) {
        // If we're already ignoring everything no need to ignore this span
        if !self.ignore.contains(&Lines::All) {
            for i in span.start().line..(span.end().line + 1) {
                self.ignore.insert(Lines::Line(i));
                if self.cover.contains(&i) {
                    self.cover.remove(&i);
                }
            }
        }
    }

    /// Cover all tokens in the given tokenstream
    pub fn cover_token_stream(&mut self, tokens: TokenStream, contents: Option<&str>) {
        for token in tokens {
            self.cover_span(token.span(), contents);
        }
    }

    /// Adds the lines of the provided span to the cover set
    pub fn cover_span(&mut self, span: Span, contents: Option<&str>) {
        // Not checking for Lines::All because I trust we've called cover_span
        // for a reason.
        let mut useful_lines: HashSet<usize> = HashSet::new();
        if let Some(ref c) = contents {
            lazy_static! {
                static ref SINGLE_LINE: Regex = Regex::new(r"\s*//").unwrap();
            }
            const MULTI_START: &str = "/*";
            const MULTI_END: &str = "*/";
            let len = span.end().line - span.start().line;
            let mut is_comment = false;
            for (i, line) in c.lines().enumerate().skip(span.start().line - 1).take(len) {
                let is_code = if line.contains(MULTI_START) {
                    if !line.contains(MULTI_END) {
                        is_comment = true;
                    }
                    false
                } else if is_comment {
                    if line.contains(MULTI_END) {
                        is_comment = false;
                    }
                    false
                } else {
                    true
                };
                if is_code && !SINGLE_LINE.is_match(line) {
                    useful_lines.insert(i + 1);
                }
            }
        }
        for i in span.start().line..(span.end().line + 1) {
            if !self.ignore.contains(&Lines::Line(i)) && useful_lines.contains(&i) {
                self.cover.insert(i);
            }
        }
    }

    /// Shows whether the line should be ignored by tarpaulin
    pub fn should_ignore(&self, line: usize) -> bool {
        self.ignore.contains(&Lines::Line(line)) || self.ignore.contains(&Lines::All)
    }

    /// Adds a line to the list of lines to ignore
    fn add_to_ignore(&mut self, lines: &[usize]) {
        if !self.ignore.contains(&Lines::All) {
            for l in lines {
                self.ignore.insert(Lines::Line(*l));
                if self.cover.contains(l) {
                    self.cover.remove(l);
                }
            }
        }
    }
}

pub struct SourceAnalysis {
    pub lines: HashMap<PathBuf, LineAnalysis>,
    pub branches: HashMap<PathBuf, BranchAnalysis>,
}

impl SourceAnalysis {
    pub fn new() -> Self {
        Self {
            lines: HashMap::new(),
            branches: HashMap::new(),
        }
    }

    pub fn get_line_analysis(&mut self, path: PathBuf) -> &mut LineAnalysis {
        self.lines.entry(path).or_insert_with(LineAnalysis::new)
    }

    pub fn get_branch_analysis(&mut self, path: PathBuf) -> &mut BranchAnalysis {
        self.branches
            .entry(path)
            .or_insert_with(BranchAnalysis::new)
    }

    pub fn get_analysis(config: &Config) -> Self {
        let mut result = Self::new();
        let mut ignored_files: HashSet<PathBuf> = HashSet::new();
        let root = config.root();

        for e in get_source_walker(config) {
            if !ignored_files.contains(e.path()) {
                result.analyse_package(e.path(), &root, &config, &mut ignored_files);
            } else {
                let mut analysis = LineAnalysis::new();
                analysis.ignore_all();
                result.lines.insert(e.path().to_path_buf(), analysis);
                ignored_files.remove(e.path());
            }
        }
        for e in &ignored_files {
            let mut analysis = LineAnalysis::new();
            analysis.ignore_all();
            result.lines.insert(e.to_path_buf(), analysis);
        }

        result.debug_printout(config);

        result
    }

    /// Analyses a package of the target crate.
    fn analyse_package(
        &mut self,
        path: &Path,
        root: &Path,
        config: &Config,
        filtered_files: &mut HashSet<PathBuf>,
    ) {
        if let Some(file) = path.to_str() {
            let skip_cause_test = config.ignore_tests && path.starts_with(root.join("tests"));
            let skip_cause_example = path.starts_with(root.join("examples"))
                && !config.run_types.contains(&RunType::Examples);
            if !(skip_cause_test || skip_cause_example) {
                let file = File::open(file);
                if let Ok(mut file) = file {
                    let mut content = String::new();
                    let res = file.read_to_string(&mut content);
                    if let Err(e) = res {
                        warn!(
                            "Unable to read file into string, skipping source analysis: {}",
                            e
                        );
                        return;
                    }
                    let file = parse_file(&content);
                    if let Ok(file) = file {
                        let ctx = Context {
                            config,
                            file_contents: &content,
                            file: path,
                            ignore_mods: RefCell::new(HashSet::new()),
                        };
                        if self.check_attr_list(&file.attrs, &ctx) {
                            self.find_ignorable_lines(&ctx);
                            self.process_items(&file.items, &ctx);

                            let mut ignored_files = ctx.ignore_mods.into_inner();
                            for f in ignored_files.drain() {
                                if f.is_file() {
                                    filtered_files.insert(f);
                                } else {
                                    let walker = WalkDir::new(f).into_iter();
                                    for e in
                                        walker.filter_map(|e| e.ok()).filter(|e| is_source_file(e))
                                    {
                                        filtered_files.insert(e.path().to_path_buf());
                                    }
                                }
                            }
                            maybe_ignore_first_line(path, &mut self.lines);
                        } else {
                            let analysis = self.get_line_analysis(path.to_path_buf());
                            analysis.ignore_span(file.span());
                        }
                    }
                }
            }
        }
    }

    /// Finds lines from the raw string which are ignorable.
    /// These are often things like close braces, semi colons that may regiser as
    /// false positives.
    fn find_ignorable_lines(&mut self, ctx: &Context) {
        lazy_static! {
            static ref IGNORABLE: Regex =
                Regex::new(r"^((\s*///)|([\[\]\{\}\(\)\s;\?,/]*$))").unwrap();
        }
        let analysis = self.get_line_analysis(ctx.file.to_path_buf());
        let lines = ctx
            .file_contents
            .lines()
            .enumerate()
            .filter(|&(_, x)| IGNORABLE.is_match(&x))
            .map(|(i, _)| i + 1)
            .collect::<Vec<usize>>();
        analysis.add_to_ignore(&lines);

        let lines = ctx
            .file_contents
            .lines()
            .enumerate()
            .filter(|&(_, x)| {
                let mut x = x.to_string();
                x.retain(|c| !c.is_whitespace());
                x == "}else{"
            })
            .map(|(i, _)| i + 1)
            .collect::<Vec<usize>>();
        analysis.add_to_ignore(&lines);
    }

    pub(crate) fn visit_generics(&mut self, generics: &Generics, ctx: &Context) {
        if let Some(ref wh) = generics.where_clause {
            let analysis = self.get_line_analysis(ctx.file.to_path_buf());
            analysis.ignore_tokens(wh);
        }
    }

    /// Printout a debug summary of the results of source analysis if debug logging
    /// is enabled
    pub fn debug_printout(&self, config: &Config) {
        if config.debug {
            for (ref path, ref analysis) in &self.lines {
                trace!(
                    "Source analysis for {}",
                    config.strip_base_dir(path).display()
                );
                let mut lines = Vec::new();
                for l in &analysis.ignore {
                    match l {
                        Lines::All => {
                            lines.clear();
                            trace!("All lines are ignorable");
                            break;
                        }
                        Lines::Line(i) => {
                            lines.push(i);
                        }
                    }
                }
                if !lines.is_empty() {
                    lines.sort();
                    trace!("Ignorable lines: {:?}", lines);
                    lines.clear()
                }
                for c in &analysis.cover {
                    lines.push(c);
                }

                if !lines.is_empty() {
                    lines.sort();
                    trace!("Coverable lines: {:?}", lines);
                }
            }
            if config.branch_coverage {
                trace!("Branch analysis");
                trace!("{:?}", self.branches);
            }
        }
    }
}

/// lib.rs:1 can often show up as a coverable line when it's not. This ignores
/// that line as long as it's not a real source line. This can also affect
/// the main files for binaries in a project as well.
fn maybe_ignore_first_line(file: &Path, result: &mut HashMap<PathBuf, LineAnalysis>) {
    if let Ok(f) = File::open(file) {
        let read_file = BufReader::new(f);
        if let Some(Ok(first)) = read_file.lines().next() {
            if !(first.starts_with("pub") || first.starts_with("fn")) {
                let file = file.to_path_buf();
                let line_analysis = result.entry(file).or_default();
                line_analysis.add_to_ignore(&[1]);
            }
        }
    }
}

/// Provides context to the source analysis stage including the tarpaulin
/// config and the source code being analysed.
pub(crate) struct Context<'a> {
    /// Program config
    config: &'a Config,
    /// Contents of the source file
    file_contents: &'a str,
    /// path to the file being analysed
    file: &'a Path,
    /// Other parts of context are immutable like tarpaulin config and users
    /// source code. This is discovered during hence use of interior mutability
    ignore_mods: RefCell<HashSet<PathBuf>>,
}
