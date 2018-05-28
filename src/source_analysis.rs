use std::cmp::{max, min};
use std::path::{PathBuf, Path};
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::ffi::OsStr;
use std::io::{Read, BufReader, BufRead};
use cargo::core::Workspace;
use syn::{*, punctuated::{Pair::End, Pair}, spanned::Spanned, punctuated::Punctuated, token::Comma};
use proc_macro2::{Span, TokenTree, TokenStream};
use regex::Regex;
use config::Config;
use walkdir::{DirEntry, WalkDir};

/// Represents the results of analysis of a single file. Does not store the file
/// in question as this is expected to be maintained by the user.
#[derive(Clone, Debug)]
pub struct LineAnalysis {
    /// This represents lines that should be ignored in coverage 
    /// but may be identifed as coverable in the DWARF tables
    pub ignore: HashSet<usize>,
    /// This represents lines that should be included in coverage
    /// But may be ignored.
    pub cover: HashSet<usize>,
}

/// When the LineAnalysis results are mapped to their files there needs to be
/// an easy way to get the information back. For the container used implement
/// this trait
pub trait SourceAnalysisQuery {
    fn should_ignore(&self, path: &Path, l:&usize) -> bool;
}

impl SourceAnalysisQuery for HashMap<PathBuf, LineAnalysis> {

    fn should_ignore(&self, path: &Path, l:&usize) -> bool {
        if self.contains_key(path) {
            self.get(path).unwrap().ignore.contains(l)
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
            cover: HashSet::new()
        }
    }

    /// Adds the lines of the provided span to the ignore set
    pub fn ignore_span(&mut self, span: &Span) {
        for i in span.start().line..(span.end().line+1) {
            self.ignore.insert(i);
            if self.cover.contains(&i) {
                self.cover.remove(&i);
            }
        }
    }

    /// Adds the lines of the provided span to the cover set
    pub fn cover_span(&mut self, span: &Span, contents: Option<&str>) {
        let mut useful_lines: HashSet<usize> = HashSet::new();
        if let Some(ref c) = contents {
            lazy_static! {
                static ref SINGLE_LINE: Regex = Regex::new(r"\s*//\n").unwrap();
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
                    useful_lines.insert(i+1);    
                }
            }
        }
        for i in span.start().line..(span.end().line +1) {
            if !self.ignore.contains(&i) && useful_lines.contains(&i) {
                self.cover.insert(i);
            }
        }
    }

    /// Shows whether the line should be ignored by tarpaulin
    pub fn should_ignore(&self, line: &usize) -> bool {
        self.ignore.contains(line)
    }
    
    /// Adds a line to the list of lines to ignore
    fn add_to_ignore(&mut self, lines: &[usize]) {
        for l in lines {
            self.ignore.insert(*l);
            if self.cover.contains(l) {
                self.cover.remove(l);
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
    let walker = WalkDir::new(project.root()).into_iter();
    for e in walker.filter_entry(|e| !is_target_folder(e, project.root()))
                   .filter_map(|e| e.ok())
                   .filter(|e| is_source_file(e)) {
        analyse_package(e.path(), project.root(), &config, &mut result); 
    }
    result
}

/// Analyse the crates lib.rs for some common false positives
fn analyse_lib_rs(file: &Path, result: &mut HashMap<PathBuf, LineAnalysis>) {
    if let Ok(f) = File::open(file) {
        let mut read_file = BufReader::new(f);
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
    config: &'a Config,
    file_contents: &'a str
}


/// Analyses a package of the target crate.
fn analyse_package(path: &Path, 
                   root: &Path,
                   config:&Config, 
                   result: &mut HashMap<PathBuf, LineAnalysis>) {

    if let Some(file) = path.to_str() {
        let skip_cause_test = config.ignore_tests && 
                              path.starts_with(root.join("tests"));
        let skip_cause_example = path.starts_with(root.join("examples"));
        if !(skip_cause_test || skip_cause_example)  {
            let file = File::open(file);
            if let Ok(mut file) =  file {
                let mut content = String::new();
                let _ = file.read_to_string(&mut content);
                let file = parse_file(&content);
                if let Ok(file) = file {
                    let mut analysis = LineAnalysis::new();
                    let ctx = Context {
                        config: config,
                        file_contents: &content,
                    };

                    find_ignorable_lines(&content, &mut analysis);
                    process_items(&file.items, &ctx, &mut analysis);
                    // Check there's no conflict!
                    result.insert(path.to_path_buf(), analysis);
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
    let lines = content.lines()
                       .enumerate()
                       .filter(|&(_, x)| !x.chars().any(|x| !"(){}[]?;\t ,".contains(x)))
                       .map(|(i, _)| i+1)
                       .collect::<Vec<usize>>();
    analysis.add_to_ignore(&lines);
}


fn process_items(items: &[Item], ctx: &Context, analysis: &mut LineAnalysis) {
    for item in items.iter() {
        match item {
            &Item::ExternCrate(ref i) => analysis.ignore_span(&i.extern_token.0),
            &Item::Use(ref i) => analysis.ignore_span(&i.span()),
            &Item::Mod(ref i) => visit_mod(&i, analysis, ctx),
            &Item::Fn(ref i) => visit_fn(&i, analysis, ctx),
            &Item::Struct(ref i) => {
                analysis.ignore_span(&i.span());
            },
            &Item::Enum(ref i) => {
                analysis.ignore_span(&i.span());
            }
            &Item::Union(ref i) => {
                analysis.ignore_span(&i.span());
            },
            &Item::Trait(ref i) => visit_trait(&i, analysis, ctx),
            &Item::Impl(ref i) => visit_impl(&i, analysis, ctx),
            &Item::Macro(ref i) => visit_macro_call(&i.mac, analysis),
            _ =>{}
        } 
    }
}


fn process_statements(stmts: &[Stmt], ctx: &Context, analysis: &mut LineAnalysis) {
    for stmt in stmts.iter() {
        match stmt {
            &Stmt::Item(ref i) => process_items(&[i.clone()], ctx, analysis),
            &Stmt::Expr(ref i) => process_expr(&i, ctx, analysis),
            &Stmt::Semi(ref i, _) => process_expr(&i, ctx, analysis),
            _ => {},
        }
    }
}


fn visit_mod(module: &ItemMod, analysis: &mut LineAnalysis, ctx: &Context) {
    analysis.ignore_span(&module.mod_token.0); 
    let mut check_insides = true;
    if ctx.config.ignore_tests {
        for attr in &module.attrs {
            if let Some(Meta::List(ref ml)) = attr.interpret_meta() {
                if ml.ident != "cfg" {
                    continue;
                }
                for nested in &ml.nested {
                    if let &NestedMeta::Meta(Meta::Word(ref i)) = nested {
                        if i == "test" {
                            check_insides = false;
                            analysis.ignore_span(&module.mod_token.0);
                            if let Some((ref braces, _)) = module.content {
                                analysis.ignore_span(&braces.0);
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
    }
}


fn visit_fn(func: &ItemFn, analysis: &mut LineAnalysis, ctx: &Context) {
    let mut test_func = false;
    let mut ignored_attr = false;
    let mut is_inline = false;
    for attr in &func.attrs {
        if let Some(x) = attr.interpret_meta() {
            let id = x.name();
            if id == "test" {
                test_func = true;
            } else if id == "derive" {
                analysis.ignore_span(&attr.bracket_token.0);
            } else if id == "inline" {
                is_inline = true;
            } else if id == "ignore" {
                ignored_attr = true;
            }
        }
    }
    if test_func && ctx.config.ignore_tests {
        if !(ignored_attr && !ctx.config.run_ignored) {
            analysis.ignore_span(&func.decl.fn_token.0);
            analysis.ignore_span(&func.block.brace_token.0);
        }
    } else {
        if is_inline {
            // We need to force cover!
            analysis.cover_span(&func.block.brace_token.0, Some(ctx.file_contents));
        }
        process_statements(&func.block.stmts, ctx, analysis);
        visit_generics(&func.decl.generics, analysis);
    }
}


fn visit_trait(trait_item: &ItemTrait, analysis: &mut LineAnalysis, ctx: &Context) {
    for item in trait_item.items.iter() {
        if let &TraitItem::Method(ref i) = item {
            if i.default.is_some() {
                analysis.cover_span(&item.span(), Some(ctx.file_contents));
                analysis.cover_span(&i.sig.decl.fn_token.0, None);
            }
        }
    }
    visit_generics(&trait_item.generics, analysis);
}


fn visit_impl(impl_blk: &ItemImpl, analysis: &mut LineAnalysis, ctx: &Context) {
    for item in impl_blk.items.iter() {
        if let &ImplItem::Method(ref i) = item {
            analysis.cover_span(&i.sig.decl.fn_token.0, None);
            analysis.cover_span(&i.block.brace_token.0, Some(ctx.file_contents));
            process_statements(&i.block.stmts, ctx, analysis);
        }
    }
    visit_generics(&impl_blk.generics, analysis);
}


fn visit_generics(generics: &Generics, analysis: &mut LineAnalysis) {
    if let Some(ref wh) = generics.where_clause {
        let span = wh.where_token.0;
        let mut lines: Vec<usize> = Vec::new();
        if span.start().column == 0 {
            lines.push(span.start().line);
        }
        for l in span.start().line+1..span.end().line +1 {
            lines.push(l);
        }
        analysis.add_to_ignore(&lines);
    }
}


fn process_expr(expr: &Expr, ctx: &Context, analysis: &mut LineAnalysis) {
    match expr {
        &Expr::Macro(ref m) => visit_macro_call(&m.mac, analysis),
        &Expr::Struct(ref s) => visit_struct_expr(&s, analysis),
        &Expr::Unsafe(ref u) => visit_unsafe_block(&u, ctx, analysis),
        &Expr::Call(ref c) => visit_callable(&c, analysis),
        &Expr::MethodCall(ref m) => visit_methodcall(&m, analysis),
        _ => {},
    }
}

fn get_coverable_args(args: &Punctuated<Expr, Comma>) -> HashSet<usize> {
    let mut lines:HashSet<usize> = HashSet::new();
    for a in args.iter() {
        let s = a.span();
        match a {
            &Expr::Lit(_) => {},
            _ => {
                for i in s.start().line..(s.end().line+1) {
                    lines.insert(i);
                }
            }
        }
    }
    lines
}


fn visit_callable(call: &ExprCall, analysis: &mut LineAnalysis ) {
    let start = call.span().start().line + 1;
    let end = call.span().end().line + 1;
    let lines = get_coverable_args(&call.args);
    let lines = (start..end).filter(|x| !lines.contains(&x))
                            .collect::<Vec<_>>();
    analysis.add_to_ignore(&lines);

}


fn visit_methodcall(meth: &ExprMethodCall, analysis: &mut LineAnalysis) {
    let start = meth.span().start().line + 1;
    let end = meth.span().end().line + 1;
    let lines = get_coverable_args(&meth.args);
    let lines = (start..end).filter(|x| !lines.contains(&x))
                            .collect::<Vec<_>>();

    analysis.add_to_ignore(&lines);
}


fn visit_unsafe_block(unsafe_expr: &ExprUnsafe, ctx: &Context, analysis: &mut LineAnalysis) {
    let u_line = unsafe_expr.unsafe_token.0.start().line;

    let blk = &unsafe_expr.block;
    if u_line != blk.brace_token.0.start().line || blk.stmts.is_empty()  {
        analysis.ignore_span(&unsafe_expr.unsafe_token.0);
    } else if let Some(ref first_stmt) = blk.stmts.iter().nth(0) {
        let s = match first_stmt {
            &&Stmt::Local(ref l) => l.span(),
            &&Stmt::Item(ref i) => i.span(),
            &&Stmt::Expr(ref e) => e.span(),
            &&Stmt::Semi(ref e, _) => e.span(),
        };
        if u_line != s.start().line {
            analysis.ignore_span(&unsafe_expr.unsafe_token.0);
        }
        process_statements(&blk.stmts, ctx, analysis); 
    } else {
        analysis.ignore_span(&unsafe_expr.unsafe_token.0);
        analysis.ignore_span(&blk.brace_token.0);
    }
}


fn visit_struct_expr(structure: &ExprStruct, analysis: &mut LineAnalysis) {
    let mut cover: HashSet<usize> = HashSet::new();
    let mut start = 0usize;
    let mut end = 0usize;
    for field in structure.fields.pairs() {
        let first = match field {
            Pair::Punctuated(t, _) => {t},
            Pair::End(t) => {t},
        };
        let span = match first.member {
            Member::Named(ref i) => i.span(),
            Member::Unnamed(ref i) => i.span,
        };
        start = min(start, span.start().line);
        end = max(start, span.start().line);
        match first.expr {
            Expr::Lit(_) | Expr::Path(_) => {},
            _=>{ 
                cover.insert(span.start().line);
            },
        }
    }
    let x = (start..(end+1)).filter(|x| !cover.contains(&x))
                            .collect::<Vec<usize>>();
    analysis.add_to_ignore(&x);
}


fn visit_macro_call(mac: &Macro, analysis: &mut LineAnalysis) {
    let mut skip = false;
    let start = mac.span().start().line + 1;
    let end = mac.span().end().line + 1;
    if let Some(End(ref name)) = mac.path.segments.last() {
        if name.ident == "unreachable" || name.ident == "unimplemented" || name.ident == "include" {
            analysis.ignore_span(&mac.span());
            skip = true;
        } 
    }
    if !skip {
        let lines = process_mac_args(&mac.tts);
        let lines = (start..end).filter(|x| !lines.contains(&x))
                                .collect::<Vec<_>>();
        analysis.add_to_ignore(&lines);
    }
}


fn process_mac_args(tokens: &TokenStream) -> HashSet<usize> {
    let mut cover: HashSet<usize> = HashSet::new();
    // IntoIter not implemented for &TokenStream.
    for token in tokens.clone().into_iter() {
        let t = token.span();
        match token {
            TokenTree::Literal(_) | TokenTree::Punct{..} => {},
            _ => { 
                for i in t.start().line..(t.end().line+1) {
                    cover.insert(i);
                }
            },
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
        assert!(!la.should_ignore(&0));
        assert!(!la.should_ignore(&10));
        
        la.add_to_ignore(&[3,4, 10]);
        assert!(la.should_ignore(&3));
        assert!(la.should_ignore(&4));
        assert!(la.should_ignore(&10));
        assert!(!la.should_ignore(&1));
    }

    #[test] 
    fn filter_str_literals() {
        let mut lines = LineAnalysis::new();
        let config = Config::default();
        let ctx = Context {
            config: &config,
            file_contents: "fn test() {\nwriteln!(#\"test\n\ttest\n\ttest\"#);\n}\n",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.len() > 1);
        assert!(lines.ignore.contains(&3));
        assert!(lines.ignore.contains(&4));
        
        let ctx = Context {
            config: &config,
            file_contents: "fn test() {\nwrite(\"test\ntest\ntest\");\n}\nfn write(s:&str){}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.len() > 1);
        assert!(lines.ignore.contains(&3));
        assert!(lines.ignore.contains(&4));
        
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "\n\nfn test() {\nwriteln!(\n#\"test\"#\n);\n}\n",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&5));
    }

    #[test]
    fn filter_struct_members() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\npub struct Struct {\npub i: i32,\nj:String,\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        
        assert!(lines.ignore.len()> 3);
        assert!(lines.ignore.contains(&1)); 
        assert!(lines.ignore.contains(&3)); 
        assert!(lines.ignore.contains(&4)); 
        
        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\npub struct Struct (\n i32\n);",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        
        assert!(!lines.ignore.is_empty());
        assert!(lines.ignore.contains(&3)); 
    }

    #[test]
    fn filter_enum_members() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\npub enum E {\nI1,\nI2(u32),\nI3{\nx:u32,\n},\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        
        assert!(lines.ignore.len()> 3);
        assert!(lines.ignore.contains(&3)); 
        assert!(lines.ignore.contains(&4)); 
        assert!(lines.ignore.contains(&5)); 
        assert!(lines.ignore.contains(&6)); 
        assert!(lines.ignore.contains(&7)); 
    }

    #[test]
    fn filter_struct_consts() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "struct T{x:String, y:i32}\nfn test()-> T {\nT{\nx:\"hello\".to_string(),\ny:4,\n}\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&5));
    }

    #[test]
    fn filter_mods() {
        let config = Config::default();
        let ctx = Context {
            config: &config,
            file_contents: "mod foo {\nfn double(x:i32)->i32 {\n x*2\n}\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&3));
        
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "mod foo;",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&1));
        
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "mod foo{}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&1));
    }

    #[test]
    fn filter_macros() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "\n\nfn unused() {\nunimplemented!();\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        
        // Braces should be ignored so number could be higher
        assert!(lines.ignore.len() >= 1);
        assert!(lines.ignore.contains(&4));
        
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "\n\nfn unused() {\nunreachable!();\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.len() >= 1);
        assert!(lines.ignore.contains(&4));
        
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn unused() {\nprintln!(\"text\");\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&2));
    }
   
    #[test]
    fn filter_tests() {
        let config = Config::default();
        let mut igconfig = Config::default();
        igconfig.ignore_tests = true;

        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[cfg(test)]\nmod tests {\n fn boo(){\nassert!(true);\n}\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&4));
        
        let ctx = Context {
            config: &igconfig,
            file_contents: "#[cfg(test)]\nmod tests {\n fn boo(){\nassert!(true);\n}\n}",
        };

        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&4));

        let ctx = Context {
            config: &config,
            file_contents: "#[test]\nfn mytest() { \n assert!(true);\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&2));
        assert!(!lines.ignore.contains(&3));

        let ctx = Context {
            config: &igconfig,
            file_contents: "#[test]\nfn mytest() { \n assert!(true);\n}",
        };
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&2));
        assert!(lines.ignore.contains(&3));
    }


    #[test]
    fn filter_where() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn boop<T>() -> T  where T:Default {\nT::default()\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&1));
        
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "fn boop<T>() -> T \nwhere T:Default {\nT::default()\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&2));
    }


    #[test]
    fn filter_derives() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "#[derive(Debug)]\nstruct T;",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&1));


        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config,
            file_contents: "\n#[derive(Copy, Eq)]\nunion x { x:i32, y:f32}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&2));
    }

    #[test]
    fn filter_unsafe() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config, 
            file_contents: "fn unsafe_fn() {\n let x=1;\nunsafe {\nprintln!(\"{}\", x);\n}\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&3));
        assert!(!lines.ignore.contains(&4));
        
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config, 
            file_contents: "fn unsafe_fn() {\n let x=1;\nunsafe {println!(\"{}\", x);}\n}",
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(!lines.ignore.contains(&3));
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
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&15));
        assert!(!lines.ignore.contains(&19));
    }

    #[test]
    fn filter_use_statements() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let ctx = Context {
            config: &config, 
            file_contents: "use std::collections::HashMap;
            use std::{ffi::CString, os::raw::c_char};"
        };
        let parser = parse_file(ctx.file_contents).unwrap();
        process_items(&parser.items, &ctx, &mut lines);
        assert!(lines.ignore.contains(&1));
        assert!(lines.ignore.contains(&2));
    }
}
