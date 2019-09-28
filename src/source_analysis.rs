use crate::config::Config;
use cargo::core::Workspace;
use lazy_static::lazy_static;
use log::trace;
use proc_macro2::{Span, TokenStream, TokenTree};
use quote::ToTokens;
use regex::Regex;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::ops::Range;
use std::path::{Path, PathBuf};
use syn::{punctuated::Pair, punctuated::Punctuated, spanned::Spanned, token::Comma, *};
use walkdir::{DirEntry, WalkDir};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Lines {
    All,
    Line(usize),
}

/// Represents the results of analysis of a single file. Does not store the file
/// in question as this is expected to be maintained by the user.
#[derive(Clone, Debug)]
pub struct LineAnalysis {
    /// This represents lines that should be ignored in coverage
    /// but may be identifed as coverable in the DWARF tables
    pub ignore: HashSet<Lines>,
    /// This represents lines that should be included in coverage
    /// But may be ignored. Doesn't make sense to cover ALL the lines so this
    /// is just an index.
    pub cover: HashSet<usize>,
}

/// When the LineAnalysis results are mapped to their files there needs to be
/// an easy way to get the information back. For the container used implement
/// this trait
pub trait SourceAnalysisQuery {
    fn should_ignore(&self, path: &Path, l: &usize) -> bool;
}

#[derive(Copy, Clone, Debug)]
enum SubResult {
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
}

impl LineAnalysis {
    /// Creates a new LineAnalysis object
    fn new() -> LineAnalysis {
        LineAnalysis {
            ignore: HashSet::new(),
            cover: HashSet::new(),
        }
    }

    pub fn ignore_all(&mut self) {
        self.ignore.clear();
        self.cover.clear();
        self.ignore.insert(Lines::All);
    }

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

fn is_source_file(entry: &DirEntry) -> bool {
    let p = entry.path();
    p.extension() == Some(OsStr::new("rs"))
}

fn is_target_folder(entry: &DirEntry, root: &Path) -> bool {
    let target = root.join("target");
    entry.path().starts_with(&target)
}

/// Returns a list of files and line numbers to ignore (not indexes!)
pub fn get_line_analysis(project: &Workspace, config: &Config) -> HashMap<PathBuf, LineAnalysis> {
    let mut result: HashMap<PathBuf, LineAnalysis> = HashMap::new();

    let mut ignored_files: HashSet<PathBuf> = HashSet::new();

    let walker = WalkDir::new(project.root()).into_iter();
    for e in walker
        .filter_entry(|e| !is_target_folder(e, project.root()))
        .filter_map(|e| e.ok())
        .filter(|e| is_source_file(e))
    {
        if !ignored_files.contains(e.path()) {
            analyse_package(
                e.path(),
                project.root(),
                &config,
                &mut result,
                &mut ignored_files,
            );
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

pub fn debug_printout(result: &HashMap<PathBuf, LineAnalysis>, config: &Config) {
    if config.debug {
        for (ref path, ref analysis) in result {
            trace!("Source analysis for {}", config.strip_base_dir(path).display());
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
        if let Some(Ok(first)) = read_file.lines().nth(0) {
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
struct Context<'a> {
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
        let skip_cause_example = path.starts_with(root.join("examples"));
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
    let lines = content
        .lines()
        .enumerate()
        .filter(|&(_, x)| !x.chars().any(|x| !"(){}[]?;\t ,".contains(x)))
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

fn process_items(items: &[Item], ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    let mut res = SubResult::Ok;
    for item in items.iter() {
        match *item {
            Item::ExternCrate(ref i) => analysis.ignore_tokens(i),
            Item::Use(ref i) => analysis.ignore_tokens(i),
            Item::Mod(ref i) => visit_mod(&i, analysis, ctx),
            Item::Fn(ref i) => visit_fn(&i, analysis, ctx),
            Item::Struct(ref i) => {
                analysis.ignore_tokens(i);
            }
            Item::Enum(ref i) => {
                analysis.ignore_tokens(i);
            }
            Item::Union(ref i) => {
                analysis.ignore_tokens(i);
            }
            Item::Trait(ref i) => visit_trait(&i, analysis, ctx),
            Item::Impl(ref i) => visit_impl(&i, analysis, ctx),
            Item::Macro(ref i) => {
                if let SubResult::Unreachable = visit_macro_call(&i.mac, ctx, analysis) {
                    res = SubResult::Unreachable;
                }
            }
            _ => {}
        }
    }
    res
}

fn process_statements(stmts: &[Stmt], ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    // in a list of statements, if any of them is unreachable, the whole list is
    // unreachable
    let mut unreachable = false;
    for stmt in stmts.iter() {
        let res = match *stmt {
            Stmt::Item(ref i) => process_items(&[i.clone()], ctx, analysis),
            Stmt::Expr(ref i) | Stmt::Semi(ref i, _) => process_expr(&i, ctx, analysis),
            _ => SubResult::Ok,
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

fn visit_mod(module: &ItemMod, analysis: &mut LineAnalysis, ctx: &Context) {
    analysis.ignore_tokens(module.mod_token);
    let mut check_insides = true;
    for attr in &module.attrs {
        if let Ok(x) = attr.parse_meta() {
            if check_cfg_attr(&x) {
                analysis.ignore_tokens(module);
                if let Some((ref braces, _)) = module.content {
                    analysis.ignore_span(braces.span);
                }
                check_insides = false;
                break;
            } else if ctx.config.ignore_tests && x.path().is_ident("cfg") {
                if let Meta::List(ref ml) = x {
                    for nested in &ml.nested {
                        if let NestedMeta::Meta(Meta::Path(ref i)) = *nested {
                            if i.is_ident("test") {
                                check_insides = false;
                                analysis.ignore_tokens(module.mod_token);
                                if let Some((ref braces, _)) = module.content {
                                    analysis.ignore_span(braces.span);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if check_insides {
        if let Some((_, ref items)) = module.content {
            process_items(items, ctx, analysis);
        }
    } else {
        // Get the file or directory name of the module
        let mut p = if let Some(parent) = ctx.file.parent() {
            parent.join(module.ident.to_string())
        } else {
            PathBuf::from(module.ident.to_string())
        };
        if !p.exists() {
            p.set_extension("rs");
        }
        ctx.ignore_mods.borrow_mut().insert(p);
    }
}

fn visit_fn(func: &ItemFn, analysis: &mut LineAnalysis, ctx: &Context) {
    let mut test_func = false;
    let mut ignored_attr = false;
    let mut is_inline = false;
    let mut ignore_span = false;
    for attr in &func.attrs {
        if let Ok(x) = attr.parse_meta() {
            let id = x.path();
            if id.is_ident("test") {
                test_func = true;
            } else if id.is_ident("derive") {
                analysis.ignore_span(attr.bracket_token.span);
            } else if id.is_ident("inline") {
                is_inline = true;
            } else if id.is_ident("ignore") {
                ignored_attr = true;
            } else if check_cfg_attr(&x) {
                ignore_span = true;
                break;
            }
        }
    }
    if ignore_span {
        analysis.ignore_tokens(func);
    } else if (test_func && ctx.config.ignore_tests) || (ignored_attr && !ctx.config.run_ignored) {
        analysis.ignore_tokens(func);
    } else {
        if is_inline {
            // We need to force cover!
            analysis.cover_span(func.block.brace_token.span, Some(ctx.file_contents));
        }
        if let SubResult::Unreachable = process_statements(&func.block.stmts, ctx, analysis) {
            // if the whole body of the function is unreachable, that means the function itself
            // cannot be called, so is unreachable as a whole
            analysis.ignore_tokens(func);
            return;
        }
        visit_generics(&func.sig.generics, analysis);
        let line_number = func.sig.fn_token.span().start().line;
        analysis.ignore.remove(&Lines::Line(line_number));
        // Ignore multiple lines of fn decl
        let decl_start = func.sig.fn_token.span().start().line + 1;
        let stmts_start = func.block.span().start().line;
        let lines = (decl_start..(stmts_start + 1)).collect::<Vec<_>>();
        analysis.add_to_ignore(&lines);
    }
}

fn check_attr_list(attrs: &[Attribute], ctx: &Context, analysis: &mut LineAnalysis) -> bool {
    let mut check_cover = true;
    for attr in attrs {
        analysis.ignore_tokens(attr);
        if let Ok(x) = attr.parse_meta() {
            if check_cfg_attr(&x) {
                check_cover = false;
            } else if ctx.config.ignore_tests && x.path().is_ident("cfg") {
                if let Meta::List(ref ml) = x {
                    let mut skip = false;
                    for c in &ml.nested {
                        if let NestedMeta::Meta(Meta::Path(ref i)) = c {
                            skip |= i.is_ident("test");
                        }
                    }
                    if skip {
                        check_cover = false;
                    }
                }
            }
        }
        if !check_cover {
            break;
        }
    }
    check_cover
}

fn check_cfg_attr(attr: &Meta) -> bool {
    let mut ignore_span = false;
    let id = attr.path();
    if id.is_ident("cfg_attr") {
        if let Meta::List(ml) = attr {
            let mut skip_match = false;
            let list = vec!["tarpaulin", "skip"];
            for (p, x) in ml.nested.iter().zip(list.iter()) {
                match p {
                    NestedMeta::Meta(Meta::Path(ref i)) => {
                        skip_match = i.is_ident(x);
                    }
                    _ => skip_match = false,
                }
                if !skip_match {
                    break;
                }
            }
            ignore_span = skip_match;
        }
    }
    ignore_span
}

fn visit_trait(trait_item: &ItemTrait, analysis: &mut LineAnalysis, ctx: &Context) {
    let check_cover = check_attr_list(&trait_item.attrs, ctx, analysis);
    if check_cover {
        for item in &trait_item.items {
            if let TraitItem::Method(ref i) = *item {
                if check_attr_list(&i.attrs, ctx, analysis) {
                    if let Some(ref block) = i.default {
                        analysis
                            .cover_token_stream(item.into_token_stream(), Some(ctx.file_contents));
                        visit_generics(&i.sig.generics, analysis);
                        analysis
                            .ignore
                            .remove(&Lines::Line(i.sig.span().start().line));

                        // Ignore multiple lines of fn decl
                        let decl_start = i.sig.fn_token.span().start().line + 1;
                        let stmts_start = block.span().start().line;
                        let lines = (decl_start..(stmts_start + 1)).collect::<Vec<_>>();
                        analysis.add_to_ignore(&lines);
                    }
                } else {
                    analysis.ignore_tokens(i);
                }
                for a in &i.attrs {
                    analysis.ignore_tokens(a);
                }
            }
        }
        visit_generics(&trait_item.generics, analysis);
    } else {
        analysis.ignore_tokens(trait_item);
    }
}

fn visit_impl(impl_blk: &ItemImpl, analysis: &mut LineAnalysis, ctx: &Context) {
    let check_cover = check_attr_list(&impl_blk.attrs, ctx, analysis);
    if check_cover {
        for item in &impl_blk.items {
            if let ImplItem::Method(ref i) = *item {
                if check_attr_list(&i.attrs, ctx, analysis) {
                    analysis.cover_token_stream(i.into_token_stream(), Some(ctx.file_contents));
                    if let SubResult::Unreachable =
                        process_statements(&i.block.stmts, ctx, analysis)
                    {
                        // if the body of this method is unreachable, this means that the method
                        // cannot be called, and is unreachable
                        analysis.ignore_tokens(i);
                        return;
                    }

                    visit_generics(&i.sig.generics, analysis);
                    analysis.ignore.remove(&Lines::Line(i.span().start().line));

                    // Ignore multiple lines of fn decl
                    let decl_start = i.sig.fn_token.span().start().line + 1;
                    let stmts_start = i.block.span().start().line;
                    let lines = (decl_start..(stmts_start + 1)).collect::<Vec<_>>();
                    analysis.add_to_ignore(&lines);
                } else {
                    analysis.ignore_tokens(item);
                }
                for a in &i.attrs {
                    analysis.ignore_tokens(a);
                }
            }
        }
        visit_generics(&impl_blk.generics, analysis);
    } else {
        analysis.ignore_tokens(impl_blk);
    }
}

fn visit_generics(generics: &Generics, analysis: &mut LineAnalysis) {
    if let Some(ref wh) = generics.where_clause {
        analysis.ignore_tokens(wh);
    }
}

fn process_expr(expr: &Expr, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
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
        // don't try to compute unreachability on other things
        _ => SubResult::Ok,
    };
    if let SubResult::Unreachable = res {
        analysis.ignore_tokens(expr);
    }
    res
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
        if let SubResult::Ok = process_expr(&arm.body, ctx, analysis) {
            reachable_arm = true
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
    // a while block is unreachable iff its body is
    if let SubResult::Unreachable = visit_block(&whl.body, ctx, analysis) {
        analysis.ignore_tokens(whl);
        SubResult::Unreachable
    } else {
        SubResult::Ok
    }
}

fn visit_for(for_loop: &ExprForLoop, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    // a for block is unreachable iff its body is
    if let SubResult::Unreachable = visit_block(&for_loop.body, ctx, analysis) {
        analysis.ignore_tokens(for_loop);
        SubResult::Unreachable
    } else {
        SubResult::Ok
    }
}

fn visit_loop(loopex: &ExprLoop, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    // a loop block is unreachable iff its body is
    if let SubResult::Unreachable = visit_block(&loopex.body, ctx, analysis) {
        analysis.ignore_tokens(loopex);
        SubResult::Unreachable
    } else {
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

fn get_line_range<T>(tokens: T) -> Range<usize>
where
    T: ToTokens,
{
    let mut start = None;
    let mut end = None;
    for token in tokens.into_token_stream() {
        let temp_start = token.span().start().line;
        let temp_end = token.span().end().line + 1;
        start = match start {
            Some(x) => Some(min(temp_start, x)),
            None => Some(temp_start),
        };
        end = match end {
            Some(x) => Some(max(temp_end, x)),
            None => Some(temp_end),
        };
    }
    match (start, end) {
        (Some(s), Some(e)) => s..e,
        _ => 0..0,
    }
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
        let start = meth.receiver.span().start().line + 1;
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

fn visit_macro_call(mac: &Macro, ctx: &Context, analysis: &mut LineAnalysis) -> SubResult {
    let mut skip = false;
    if let Some(PathSegment {
        ref ident,
        arguments: _,
    }) = mac.path.segments.last()
    {
        let unreachable = ident == "unreachable";
        let standard_ignores = ident == "unimplemented" || ident == "include" || ident == "cfg";
        let ignore_panic = ctx.config.ignore_panics && ident == "panic";
        if standard_ignores || ignore_panic || unreachable {
            analysis.ignore_tokens(mac);
            skip = true;
        }
        if unreachable {
            return SubResult::Unreachable;
        }
    }
    if !skip {
        let start = mac.span().start().line + 1;
        let range = get_line_range(mac);
        let lines = process_mac_args(&mac.tokens);
        let lines = (start..range.end)
            .filter(|x| !lines.contains(&x))
            .collect::<Vec<_>>();
        analysis.add_to_ignore(&lines);
    }
    SubResult::Ok
}

fn process_mac_args(tokens: &TokenStream) -> HashSet<usize> {
    let mut cover: HashSet<usize> = HashSet::new();
    // IntoIter not implemented for &TokenStream.
    for token in tokens.clone() {
        match token {
            TokenTree::Literal(_) | TokenTree::Punct { .. } => {}
            _ => {
                for i in get_line_range(token) {
                    cover.insert(i);
                }
            }
        }
    }
    cover
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_file;

    #[test]
    fn line_analysis_works() {
        let mut la = LineAnalysis::new();
        assert!(!la.should_ignore(0));
        assert!(!la.should_ignore(10));

        la.add_to_ignore(&[3, 4, 10]);
        assert!(la.should_ignore(3));
        assert!(la.should_ignore(4));
        assert!(la.should_ignore(10));
        assert!(!la.should_ignore(1));
    }

    #[test]
    fn filter_str_literals() {
        let mut lines = LineAnalysis::new();
        let config = Config::default();
        let ctx = Context {
            config: &config,
            file_contents: "fn test() {
                writeln!(#\"test
                         \ttest
                         \ttest\"#);
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.len() > 1);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));

        let ctx = Context {
            config: &config,
            file_contents: "fn test() {
                write(\"test
                      test
                      test\");
            }
            fn write(s:&str){}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.len() > 1);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "

                fn test() {
                    writeln!(
                        #\"test\"#
                        );
                }
            ",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(5)));
    }

    #[test]
    fn filter_struct_members() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\npub struct Struct {\npub i: i32,\nj:String,\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);

        assert!(lines.ignore.len() > 3);
        assert!(lines.ignore.contains(&Lines::Line(1)));
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));

        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\npub struct Struct (\n i32\n);",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);

        assert!(!lines.ignore.is_empty());
        assert!(lines.ignore.contains(&Lines::Line(3)));
    }

    #[test]
    fn filter_enum_members() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\npub enum E {\nI1,\nI2(u32),\nI3{\nx:u32,\n},\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);

        assert!(lines.ignore.len() > 3);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(5)));
        assert!(lines.ignore.contains(&Lines::Line(6)));
        assert!(lines.ignore.contains(&Lines::Line(7)));
    }

    #[test]
    fn filter_struct_consts() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct T{x:String, y:i32}
                fn test()-> T {
                    T{
                        x:String::from(\"hello\"), //function call should be covered
                        y:4,
                    }
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(5)));
    }

    #[test]
    fn filter_unreachable_unchecked() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn test() {
                    core::hint::unreachable_unchecked();
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));
    }

    #[test]
    fn filter_mods() {
        let config = Config::default();
        let ctx = Context {
            config: &config,
            file_contents: "mod foo {\nfn double(x:i32)->i32 {\n x*2\n}\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(3)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "mod foo;",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(1)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "mod foo{}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(1)));
    }

    #[test]
    fn filter_macros() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "\n\nfn unused() {\nunimplemented!();\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);

        // Braces should be ignored so number could be higher
        assert!(lines.ignore.len() >= 1);
        assert!(lines.ignore.contains(&Lines::Line(4)));
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "\n\nfn unused() {\nunreachable!();\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.len() >= 1);
        assert!(lines.ignore.contains(&Lines::Line(4)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unreachable_match(x: u32) -> u32 {
                match x {
                    1 => 5,
                    2 => 7,
                    _ => unreachable!(),
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(5)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unused() {\nprintln!(\"text\");\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(2)));
    }

    #[test]
    fn filter_tests() {
        let config = Config::default();
        let mut igconfig = Config::default();
        igconfig.ignore_tests = true;

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[cfg(test)]
                mod tests {
                    fn boo(){
                        assert!(true);
                    }\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(4)));

        let ctx = Context {
            config: &igconfig,
            file_contents: "#[cfg(test)]
                mod tests {
                    fn boo(){
                        assert!(true);
                    }
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };

        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(4)));

        let ctx = Context {
            config: &config,
            file_contents: "#[test]\nfn mytest() { \n assert!(true);\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(2)));
        assert!(!lines.ignore.contains(&Lines::Line(3)));

        let ctx = Context {
            config: &igconfig,
            file_contents: "#[test]\nfn mytest() { \n assert!(true);\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));
        assert!(lines.ignore.contains(&Lines::Line(3)));
    }

    #[test]
    fn filter_test_utilities() {
        let mut config = Config::default();
        config.ignore_tests = true;

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "trait Thing {
                #[cfg(test)]
                fn boo(){
                    assert!(true);
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));

        let config = Config::default();

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "trait Thing {
                #[cfg(test)]
                fn boo(){
                    assert!(true);
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(3)));
        assert!(!lines.ignore.contains(&Lines::Line(4)));
    }

    #[test]
    fn filter_where() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn boop<T>() -> T  where T:Default {
                T::default()
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(1)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn boop<T>() -> T
                where T:Default {
                    T::default()
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "trait foof {
                fn boop<T>() -> T
                where T:Default {
                    T::default()
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(3)));
    }

    #[test]
    fn filter_derives() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\nstruct T;",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(1)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "\n#[derive(Copy, Eq)]\nunion x { x:i32, y:f32}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));
    }

    #[test]
    fn filter_unsafe() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unsafe_fn() {\n let x=1;\nunsafe {\nprintln!(\"{}\", x);\n}\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(!lines.ignore.contains(&Lines::Line(4)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unsafe_fn() {\n let x=1;\nunsafe {println!(\"{}\", x);}\n}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(3)));
    }

    #[test]
    fn cover_generic_impl_methods() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct GenericStruct<T>(T);
            impl<T> GenericStruct<T> {
                fn hw(&self) {
                    println!(\"hello world\");
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.cover.contains(&3));
        assert!(lines.cover.contains(&4));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct GenericStruct<T>{v:Vec<T>}
            impl<T> Default for GenericStruct<T> {
                fn default() -> Self {
                    T {
                        v: vec![],
                    }
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.cover.contains(&5));
    }

    #[test]
    fn cover_default_trait_methods() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "trait Thing {
                fn hw(&self) {
                    println!(\"hello world\");
                    }
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.cover.contains(&2));
        assert!(lines.cover.contains(&3));
    }

    #[test]
    fn filter_method_args() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct Thing;
            impl Thing{
                fn hw(&self, name: &str) {
                    println!(\"hello {}\", name);
                }                                           //5
            }

            fn get_name() -> String {
                return \"Daniel\".to_string()
            }                                               //10

            fn main() {
                let s=Thing{};
                s.hw(
                    \"Paul\"                                //15
                );

                s.hw(
                    &get_name()
                );                                          //20
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(15)));
        assert!(!lines.ignore.contains(&Lines::Line(19)));
    }

    #[test]
    fn filter_use_statements() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "use std::collections::HashMap;
            use std::{ffi::CString, os::raw::c_char};",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(1)));
        assert!(lines.ignore.contains(&Lines::Line(2)));
    }

    #[test]
    fn include_inline_fns() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[inline]
                fn inline_func() {
                    // I shouldn't be covered
                    println!(\"I should\");
                    /*
                     None of us should
                     */
                    println!(\"But I will\");
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.cover.contains(&3));
        assert!(lines.cover.contains(&4));
        assert!(!lines.cover.contains(&5));
        assert!(!lines.cover.contains(&6));
        assert!(!lines.cover.contains(&7));
        assert!(lines.cover.contains(&8));
    }

    #[test]
    fn cover_callable_noargs() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn foo() {
                    std::ptr::null::<i32>();
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(2)));
    }

    #[test]
    fn filter_closure_contents() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn inline_func() {
                    (0..0).iter().foreach(|x| {
                        unreachable!();
                        });
                }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(3)));
    }

    #[test]
    fn tarpaulin_skip_attr() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[cfg_attr(tarpaulin, skip)]
                fn skipped() {
                    println!(\"Hello world\");
                }

            #[cfg_attr(tarpaulin, not_a_thing)]
            fn covered() {
                println!(\"hell world\");
            }
            ",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(!lines.ignore.contains(&Lines::Line(7)));
        assert!(!lines.ignore.contains(&Lines::Line(8)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[cfg_attr(tarpaulin, skip)]
            mod ignore_all {
                fn skipped() {
                    println!(\"Hello world\");
                }

                #[cfg_attr(tarpaulin, not_a_thing)]
                fn covered() {
                    println!(\"hell world\");
                }
            }
            ",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(8)));
        assert!(lines.ignore.contains(&Lines::Line(9)));
    }

    #[test]
    fn tarpaulin_skip_trait_attrs() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[cfg_attr(tarpaulin, skip)]
                trait Foo {
                    fn bar() {
                        println!(\"Hello world\");
                    }


                    fn not_covered() {
                        println!(\"hell world\");
                    }
                }
            ",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(8)));
        assert!(lines.ignore.contains(&Lines::Line(9)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "trait Foo {
                    fn bar() {
                        println!(\"Hello world\");
                    }

                    #[cfg_attr(tarpaulin, skip)]
                    fn not_covered() {
                        println!(\"hell world\");
                    }
                }
            ",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(2)));
        assert!(!lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(7)));
        assert!(lines.ignore.contains(&Lines::Line(8)));
    }

    #[test]
    fn tarpaulin_skip_impl_attrs() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct Foo;
                #[cfg_attr(tarpaulin, skip)]
                impl Foo {
                    fn bar() {
                        println!(\"Hello world\");
                    }


                    fn not_covered() {
                        println!(\"hell world\");
                    }
                }
            ",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(5)));
        assert!(lines.ignore.contains(&Lines::Line(9)));
        assert!(lines.ignore.contains(&Lines::Line(10)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct Foo;
                impl Foo {
                    fn bar() {
                        println!(\"Hello world\");
                    }


                    #[cfg_attr(tarpaulin, skip)]
                    fn not_covered() {
                        println!(\"hell world\");
                    }
                }
            ",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(3)));
        assert!(!lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(9)));
        assert!(lines.ignore.contains(&Lines::Line(10)));
    }

    #[test]
    fn filter_block_contents() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unreachable_match(x: u32) -> u32 {
                match x {
                    1 => 5,
                    2 => 7,
                    _ => {
                        unreachable!();
                    },
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(6)));
    }

    #[test]
    fn optional_panic_ignore() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unreachable_match(x: u32) -> u32 {
                match x {
                    1 => 5,
                    2 => 7,
                    _ => panic!(),
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&Lines::Line(5)));

        let mut config = Config::default();
        config.ignore_panics = true;
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unreachable_match(x: u32) -> u32 {
                match x {
                    1 => 5,
                    2 => 7,
                    _ => panic!(),
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };

        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(5)));
    }

    #[test]
    fn filter_nested_blocks() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn block() {
                {
                    loop {
                        for i in 1..2 {
                            if false {
                                while let Some(x) = Some(6) {
                                    while false {
                                        if let Ok(y) = Ok(4) {
                                            unreachable!();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(9)));
    }

    #[test]
    fn filter_multi_line_decls() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn print_it(x:u32,
                y:u32,
                z:u32) {
                println!(\"{}:{}:{}\",x,y,z);
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));
        assert!(lines.ignore.contains(&Lines::Line(3)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct Boo;
            impl Boo {
                fn print_it(x:u32,
                    y:u32,
                    z:u32) {
                    println!(\"{}:{}:{}\",x,y,z);
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(5)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "trait Boo {
                fn print_it(x:u32,
                    y:u32,
                    z:u32) {
                    println!(\"{}:{}:{}\",x,y,z);
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));
    }

    #[test]
    fn unreachable_propagate() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "enum Void {}
            fn empty_match(x: Void) -> u32 {
                match x {
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(2)));
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(5)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn foo() {
                if random() {
                    loop {
                        match random() {
                            true => match void() {},
                            false => unreachable!()
                        }
                    }
                } else {
                    call();
                }
            }",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(5)));
        assert!(lines.ignore.contains(&Lines::Line(6)));
        assert!(lines.ignore.contains(&Lines::Line(7)));
        assert!(lines.ignore.contains(&Lines::Line(8)));

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn test_unreachable() {
				let x: u32 = foo();
				if x > 5 {
					bar();
				}
				unreachable!();
			}",
            file: Path::new(""),
            ignore_mods: RefCell::new(HashSet::new()),
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&Lines::Line(1)));
        assert!(lines.ignore.contains(&Lines::Line(2)));
        assert!(lines.ignore.contains(&Lines::Line(3)));
        assert!(lines.ignore.contains(&Lines::Line(4)));
        assert!(lines.ignore.contains(&Lines::Line(5)));
        assert!(lines.ignore.contains(&Lines::Line(6)));
        assert!(lines.ignore.contains(&Lines::Line(7)));
    }

}
