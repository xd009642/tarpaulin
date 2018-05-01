use std::cmp::max;
use std::path::{PathBuf, Path};
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::io::{BufReader, BufRead};
use cargo::core::{Workspace, Package};
use cargo::sources::PathSource;
use cargo::util::Config as CargoConfig;
use syn::{*, punctuated::Pair::End};
use proc_macro2::{Span, TokenTree, TokenStream};
use regex::Regex;
use config::Config;

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
        let mut useless_lines: HashSet<usize> = HashSet::new();
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
                    useless_lines.insert(i+1);    
                }
            }
        }
        for i in span.start().line..(span.end().line +1) {
            if !self.ignore.contains(&i) && !useless_lines.contains(&i) {
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

    /// Adds a line to the list of lines to cover
    fn add_to_cover(&mut self, lines: &[usize]) {
        for l in lines {
            if !self.ignore.contains(l) {
                self.cover.insert(*l);
            }
        }
    }
}

/// Returns a list of files and line numbers to ignore (not indexes!)
pub fn get_line_analysis(project: &Workspace, config: &Config) -> HashMap<PathBuf, LineAnalysis> {
    let mut result: HashMap<PathBuf, LineAnalysis> = HashMap::new();
    // Members iterates over all non-virtual packages in the workspace
    for pkg in project.members() {
        if config.packages.is_empty() || config.packages.contains(&pkg.name().to_string()) {
            analyse_package(pkg, &config, project.config(), &mut result); 
        }
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
fn analyse_package(pkg: &Package, 
                   config:&Config, 
                   cargo_conf: &CargoConfig, 
                   result: &mut HashMap<PathBuf, LineAnalysis>) {

    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), cargo_conf);
    if let Ok(package) = src.root_package() {
        for target in package.targets() {
            let path = target.src_path();
            let file = match path.to_str() {
                Some(s) => s,
                _ => continue
            };
            let skip_cause_test = config.ignore_tests && 
                                  path.starts_with(pkg.root().join("tests"));
            let skip_cause_example = path.starts_with(pkg.root().join("examples"));
            if !(skip_cause_test || skip_cause_example)  {
                let file = File::open(file);
                let mut file = match file {
                    Ok(f) => f,
                    _ => continue,
                };
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
                }
            }
            // This could probably be done with the DWARF if I could find a discriminating factor
            // to why lib.rs:1 shows up as a real line!
            if file.ends_with("src/lib.rs") {
                analyse_lib_rs(path, result);
            }
        }
    }
}


fn find_ignorable_lines(content: &str, analysis: &mut LineAnalysis) {
    let lines = content.lines()
                       .enumerate()
                       .filter(|(_, x)| !x.chars().any(|x| !"(){}[]?;\t ,".contains(x)))
                       .map(|(i, _)| i+1)
                       .collect::<Vec<usize>>();
    analysis.add_to_ignore(&lines);
}


fn process_items(items: &[Item], ctx: &Context, analysis: &mut LineAnalysis) {
    for item in items {
        match item {
            Item::ExternCrate(i) => analysis.ignore_span(&i.extern_token.0),
            Item::Use(i) => analysis.ignore_span(&i.use_token.0),
            Item::Mod(i) => visit_mod(i, analysis, ctx),
            Item::Fn(i) => visit_fn(i, analysis, ctx),
            Item::Struct(i) => visit_struct(i, analysis),
            Item::Enum(i) => visit_enum(i, analysis),
            Item::Union(i) => visit_union(i, analysis),
            Item::Trait(i) => visit_trait(i, analysis, ctx),
            Item::Impl(i) => visit_impl(i, analysis, ctx),
            Item::Macro(i) => visit_macro_call(&i.mac, analysis),
            _ =>{}
        } 
    }
}


fn process_statements(stmts: &[Stmt], ctx: &Context, analysis: &mut LineAnalysis) {
    for stmt in stmts {
        match stmt {
            Stmt::Item(i) => process_items(&[i.clone()], ctx, analysis),
            Stmt::Expr(i) => process_expr(i, analysis),
            Stmt::Semi(i, _) => process_expr(i, analysis),
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
                if ml.ident != Ident::from("cfg") {
                    continue;
                }
                for nested in &ml.nested {
                    if let NestedMeta::Meta(Meta::Word(i)) = nested {
                        if i == &Ident::from("test") {
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
    let mut ignore_func = false;
    let mut is_inline = false;
    for attr in &func.attrs {
        if let Some(x) = attr.interpret_meta() {
            let id = x.name();
            if id == Ident::from("test") {
                ignore_func = true;
            } else if id == Ident::from("derive") {
                analysis.ignore_span(&attr.bracket_token.0);
            } else if id == Ident::from("inline") {
                is_inline = true;
            }
        }
    }
    if ignore_func && ctx.config.ignore_tests {
        analysis.ignore_span(&func.decl.fn_token.0);
        analysis.ignore_span(&func.block.brace_token.0);
    } else {
        if is_inline {
            // We need to force cover!
            analysis.cover_span(&func.block.brace_token.0, Some(ctx.file_contents));
        }
        process_statements(&func.block.stmts, ctx, analysis);
        visit_generics(&func.decl.generics, analysis);
    }
}


fn visit_struct(structure: &ItemStruct, analysis: &mut LineAnalysis) {
    for field in &structure.fields {
        if let Some(colon) = field.colon_token {
            analysis.ignore_span(&colon.0[0]);
        }
    }
    ignore_derive_attrs(&structure.attrs, analysis);
    visit_generics(&structure.generics, analysis);
}


fn visit_trait(trait_item: &ItemTrait, analysis: &mut LineAnalysis, ctx: &Context) {
    for item in &trait_item.items {
        if let TraitItem::Method(ref i) = item {
            if let Some(ref default_impl) = i.default {
                analysis.cover_span(&i.sig.decl.fn_token.0, None);
                analysis.cover_span(&default_impl.brace_token.0, Some(ctx.file_contents));
            }
        }
    }
    visit_generics(&trait_item.generics, analysis);
}


fn visit_impl(impl_blk: &ItemImpl, analysis: &mut LineAnalysis, ctx: &Context) {
    for item in &impl_blk.items {
        if let ImplItem::Method(ref i) = item {
            analysis.cover_span(&i.sig.decl.fn_token.0, None);
            analysis.cover_span(&i.block.brace_token.0, Some(ctx.file_contents));
            process_statements(&i.block.stmts, ctx, analysis);
        }
    }
    visit_generics(&impl_blk.generics, analysis);
}


fn visit_enum(enumeration: &ItemEnum, analysis: &mut LineAnalysis) {
    ignore_derive_attrs(&enumeration.attrs, analysis);
    visit_generics(&enumeration.generics, analysis);
}


fn visit_union(uni: &ItemUnion, analysis: &mut LineAnalysis) {
    ignore_derive_attrs(&uni.attrs, analysis);
    visit_generics(&uni.generics, analysis);
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


fn ignore_derive_attrs(attrs: &[Attribute], analysis: &mut LineAnalysis) {
    for attr in attrs {
        if let Some(x) = attr.interpret_meta() {
            if x.name() == Ident::from("derive") {
                analysis.ignore_span(&attr.bracket_token.0);
            }
        }
    }
}


fn process_expr(expr: &Expr, analysis: &mut LineAnalysis) {
    match expr {
        Expr::Macro(m) => visit_macro_call(&m.mac, analysis),
        _ => {},
    }
}


fn visit_macro_call(mac: &Macro, analysis: &mut LineAnalysis) {
    if let Some(End(ref name)) = mac.path.segments.last() {
        if name.ident == Ident::from("unreachable") || name.ident == Ident::from("unimplemented") || name.ident == Ident::from("include") {
            analysis.ignore_span(&name.ident.span());
        } else {
            // This could be outside but it wouldn't make sense for it to not have a end segment
            // but have args 
            process_mac_args(&mac.tts, name.ident.span().start().line, analysis);
        }
    } 
}


fn process_mac_args(tokens: &TokenStream, first_line: usize, analysis: &mut LineAnalysis) {
    let mut cover: HashSet<usize> = HashSet::new();
    let mut end_line: usize = first_line;
    // IntoIter not implemented for &TokenStream.
    for token in tokens.clone().into_iter() {
        end_line = max(end_line, token.span().end().line);
        match token {
            TokenTree::Literal(_) => {},
            t @ _ => { 
                cover.insert(t.span().start().line);
                for i in t.span().start().line..(t.span().end().line+1) {
                    cover.insert(i);
                }
            },
        }
    }
    let lines:Vec<usize> = ((first_line+1)..(end_line+1)).filter(|x| !cover.contains(&x))
                                                         .collect();
    analysis.add_to_ignore(&lines);
}


#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_file;

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
        
        assert_eq!(lines.ignore.len(), 3);
        assert!(lines.ignore.contains(&1)); 
        assert!(lines.ignore.contains(&3)); 
        assert!(lines.ignore.contains(&4)); 
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
}
