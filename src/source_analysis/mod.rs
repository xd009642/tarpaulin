use crate::config::{Config, RunType};
use crate::path_utils::{get_source_walker, is_source_file};
use items::process_items;
use lazy_static::lazy_static;
use log::trace;
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use syn::*;
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
    pub(crate) use expressions::*;
    pub(crate) use macros::*;
    pub(crate) use statements::*;
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
                static ref MULTI_START: Regex = Regex::new(r"/\*").unwrap();
                static ref MULTI_END: Regex = Regex::new(r"\*/").unwrap();
            }
            let len = span.end().line - span.start().line;
            let mut is_comment = false;
            for (i, line) in c.lines().enumerate().skip(span.start().line - 1).take(len) {
                let is_code = if MULTI_START.is_match(line) {
                    if !MULTI_END.is_match(line) {
                        is_comment = true;
                    }
                    false
                } else if is_comment {
                    if MULTI_END.is_match(line) {
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

/// Returns a list of files and line numbers to ignore (not indexes!)
pub fn get_line_analysis(config: &Config) -> HashMap<PathBuf, LineAnalysis> {
    let mut result: HashMap<PathBuf, LineAnalysis> = HashMap::new();

    let mut ignored_files: HashSet<PathBuf> = HashSet::new();
    let root = config.root();

    for e in get_source_walker(config) {
        if !ignored_files.contains(e.path()) {
            analyse_package(e.path(), &root, &config, &mut result, &mut ignored_files);
        } else {
            let mut analysis = LineAnalysis::new();
            analysis.ignore_all();
            result.insert(e.path().to_path_buf(), analysis);
            ignored_files.remove(e.path());
        }
    }
    for e in &ignored_files {
        let mut analysis = LineAnalysis::new();
        analysis.ignore_all();
        result.insert(e.to_path_buf(), analysis);
    }

    debug_printout(&result, config);

    result
}

/// Printout a debug summary of the results of source analysis if debug logging
/// is enabled
pub fn debug_printout(result: &HashMap<PathBuf, LineAnalysis>, config: &Config) {
    if config.debug {
        for (ref path, ref analysis) in result {
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
    }
}

/// Analyse the crates lib.rs for some common false positives
fn analyse_lib_rs(file: &Path, result: &mut HashMap<PathBuf, LineAnalysis>) {
    if let Ok(f) = File::open(file) {
        let read_file = BufReader::new(f);
        if let Some(Ok(first)) = read_file.lines().next() {
            if !(first.starts_with("pub") || first.starts_with("fn")) {
                let file = file.to_path_buf();
                if result.contains_key(&file) {
                    let l = result.get_mut(&file).unwrap();
                    l.add_to_ignore(&[1]);
                } else {
                    let mut l = LineAnalysis::new();
                    l.add_to_ignore(&[1]);
                    result.insert(file, l);
                }
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

/// Analyses a package of the target crate.
fn analyse_package(
    path: &Path,
    root: &Path,
    config: &Config,
    result: &mut HashMap<PathBuf, LineAnalysis>,
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
                let _ = file.read_to_string(&mut content);
                let file = parse_file(&content);
                if let Ok(file) = file {
                    let mut analysis = LineAnalysis::new();
                    let ctx = Context {
                        config,
                        file_contents: &content,
                        file: path,
                        ignore_mods: RefCell::new(HashSet::new()),
                    };

                    find_ignorable_lines(&content, &mut analysis);
                    process_items(&file.items, &ctx, &mut analysis);
                    // Check there's no conflict!
                    result.insert(path.to_path_buf(), analysis);

                    let mut ignored_files = ctx.ignore_mods.into_inner();
                    for f in ignored_files.drain() {
                        if f.is_file() {
                            filtered_files.insert(f);
                        } else {
                            let walker = WalkDir::new(f).into_iter();
                            for e in walker.filter_map(|e| e.ok()).filter(|e| is_source_file(e)) {
                                filtered_files.insert(e.path().to_path_buf());
                            }
                        }
                    }
                    // This could probably be done with the DWARF if I could find a discriminating factor
                    // to why lib.rs:1 shows up as a real line!
                    if path.ends_with("src/lib.rs") {
                        analyse_lib_rs(path, result);
                    }
                }
            }
        }
    }
}

/// Finds lines from the raw string which are ignorable.
/// These are often things like close braces, semi colons that may regiser as
/// false positives.
fn find_ignorable_lines(content: &str, analysis: &mut LineAnalysis) {
    lazy_static! {
        static ref IGNORABLE: Regex = Regex::new(r"^((\s*///)|([\[\]\{\}\(\)\s;\?,/]*$))").unwrap();
    }
    let lines = content
        .lines()
        .enumerate()
        .filter(|&(_, x)| IGNORABLE.is_match(&x))
        .map(|(i, _)| i + 1)
        .collect::<Vec<usize>>();
    analysis.add_to_ignore(&lines);

    let lines = content
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

pub fn visit_generics(generics: &Generics, analysis: &mut LineAnalysis) {
    if let Some(ref wh) = generics.where_clause {
        analysis.ignore_tokens(wh);
    }
}
