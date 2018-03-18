use std::rc::Rc;
use std::ops::Deref;
use std::path::{PathBuf, Path};
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::{BufReader, BufRead};
use cargo::core::{Workspace, Package};
use cargo::sources::PathSource;
use cargo::util::Config as CargoConfig;
use syntex_syntax::attr;
use syntex_syntax::visit::{self, Visitor, FnKind};
use syntex_syntax::codemap::{CodeMap, Span, FilePathMapping};
use syntex_syntax::ast::*;
use syntex_syntax::parse::{self, ParseSess};
use syntex_syntax::parse::token::*;
use syntex_syntax::tokenstream::TokenTree;
use syntex_syntax::errors::Handler;
use syntex_syntax::errors::emitter::ColorConfig;
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
    fn new() -> LineAnalysis {
        LineAnalysis {
            ignore: HashSet::new(),
            cover: HashSet::new()
        }
    }

    pub fn should_ignore(&self, line: &usize) -> bool {
        self.ignore.contains(line)
    }
    
    fn add_to_ignore(&mut self, lines: &[usize]) {
        for l in lines {
            self.ignore.insert(*l);
            if self.cover.contains(l) {
                self.cover.remove(l);
            }
        }
    }

    fn add_to_cover(&mut self, lines: &[usize]) {
        for l in lines {
            if !self.ignore.contains(l) {
                self.cover.insert(*l);
            }
        }
    }
}

struct CoverageVisitor<'a> {
    lines: Vec<(PathBuf, usize)>,
    coverable: Vec<(PathBuf, usize)>,
    covered: &'a HashSet<PathBuf>,
    codemap: &'a CodeMap,
    config: &'a Config, 
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

fn analyse_package(pkg: &Package, 
                   config:&Config, 
                   cargo_conf: &CargoConfig, 
                   result: &mut HashMap<PathBuf, LineAnalysis>) {

    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), cargo_conf);
    if let Ok(package) = src.root_package() {

        let codemap = Rc::new(CodeMap::new(FilePathMapping::empty()));
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, false, false, Some(codemap.clone()));
        let parse_session = ParseSess::with_span_handler(handler, codemap.clone());
        
        for target in package.targets() {
            let file = target.src_path();
            let skip_cause_test = config.ignore_tests && 
                                  file.starts_with(pkg.root().join("tests"));
            let skip_cause_example = file.starts_with(pkg.root().join("examples"));
            if !(skip_cause_test || skip_cause_example) {
                let mut parser = parse::new_parser_from_file(&parse_session, file);
                parser.cfg_mods = false;
                if let Ok(krate) = parser.parse_crate_mod() {
                    
                    let done_files: HashSet<PathBuf> = result.keys()
                                                             .map(|x| x.clone())
                                                             .collect::<HashSet<_>>();
                    let lines = {
                        let mut visitor = CoverageVisitor::from_session(&parse_session, &done_files, config);
                        visitor.visit_mod(&krate.module, krate.span, &krate.attrs, NodeId::new(0));
                        visitor
                    };
                    for ignore in &lines.lines {
                        if result.contains_key(&ignore.0) {
                            let l = result.get_mut(&ignore.0).unwrap();
                            l.add_to_ignore(&[ignore.1]);
                        }
                        else {
                            let mut l = LineAnalysis::new();
                            l.add_to_ignore(&[ignore.1]);
                            result.insert(ignore.0.clone(), l);                         
                        }
                    }
                    for cover in &lines.coverable {
                        if result.contains_key(&cover.0) {
                            let l = result.get_mut(&cover.0).unwrap();
                            l.add_to_cover(&[cover.1]);
                        }
                        else {
                            let mut l = LineAnalysis::new();
                            l.add_to_cover(&[cover.1]);
                            result.insert(cover.0.clone(), l);                         
                        }
                    }
                }
            }
            // This could probably be done with the DWARF if I could find a discriminating factor
            // to why lib.rs:1 shows up as a real line!
            if file.ends_with("src/lib.rs") {
                analyse_lib_rs(file, result);
            }
        }
    }
}


impl<'a> CoverageVisitor<'a> {
    /// Construct a new ignored lines object for the given project
    fn from_session(session: &'a ParseSess, 
                    covered: &'a HashSet<PathBuf>, 
                    config: &'a Config) -> CoverageVisitor<'a> {
        CoverageVisitor {
            lines: vec![],
            coverable: vec![],
            covered: covered,
            codemap: session.codemap(),
            config: config,
        }
    }
    
    fn get_line_indexes(&mut self, span: Span) -> Vec<usize> {
        if let Ok(ts) = self.codemap.span_to_lines(span) {
            ts.lines.iter()
                    .map(|x| x.line_index)
                    .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }

    /// Add lines to the line ignore list
    fn ignore_lines(&mut self, span: Span) {
        if let Ok(ls) = self.codemap.span_to_lines(span) {
            for line in &ls.lines {
                let pb = PathBuf::from(self.codemap.span_to_filename(span) as String);
                // Line number is index+1
                self.lines.push((pb, line.line_index + 1));
            }
        }
    }    


    fn cover_lines(&mut self, span: Span) {
        if let Ok(ls) = self.codemap.span_to_lines(span) {
            let temp_string = self.codemap.span_to_string(span);
            let txt = temp_string.lines();
            let mut is_comment = false;
            lazy_static! {
                static ref SINGLE_LINE: Regex = Regex::new(r"\s*//\n").unwrap();
                static ref MULTI_START: Regex = Regex::new(r"/\*").unwrap();
                static ref MULTI_END: Regex = Regex::new(r"\*/").unwrap();
            }
            for (&line, text) in ls.lines.iter().zip(txt) {
                let is_code = if MULTI_START.is_match(text) {
                    if !MULTI_END.is_match(text) {
                        is_comment = true;
                    } 
                    false
                } else if is_comment {
                    if MULTI_END.is_match(text) {
                        is_comment = false;
                    }
                    false
                } else {
                    true
                };
                if is_code && !SINGLE_LINE.is_match(text) {
                    let pb = PathBuf::from(self.codemap.span_to_filename(span) as String);
                    // Line number is index+1
                    self.coverable.push((pb, line.line_index + 1));
                }
            }
        }
    }

   
    /// Looks for #[cfg(test)] attribute.
    fn contains_cfg_test(&mut self, attrs: &[Attribute]) -> bool {
        attrs.iter()
             .filter(|x| x.path == "cfg")
             .filter_map(|x| x.meta_item_list())
             .flat_map(|x| x)
             .any(|x| { 
                 if let Some(w) = x.word() {
                    w.name().as_str() == "test"
                 } else {
                     false
                 }
             })
    }

    /// This function finds ignorable lines within actual coverable code. 
    /// As opposed to other functions which find isolated lines that aren't 
    /// executed or lines filtered by the user. These lines are things like 
    /// close braces that are within coverable code but not coverable.
    fn find_ignorable_lines(&mut self, span: Span) {
        if let Ok(l) = self.codemap.span_to_lines(span) {
            for line in &l.lines {
                let pb = PathBuf::from(self.codemap.span_to_filename(span) as String);
                if let Some(s) = l.file.get_line(line.line_index) {
                    // Is this one of those pointless {, } or }; or )?; only lines?
                    if !s.chars().any(|x| !"(){}[]?;\t ,".contains(x)) {
                        self.lines.push((pb, line.line_index + 1));
                    }
                }
            }
        }
    }

    fn ignore_mac_args(&mut self, mac: &Mac_, s:Span) {
        let mut cover: HashSet<usize>  = HashSet::new();
        for token in mac.stream().into_trees() {
            match token {
                TokenTree::Token(ref s, ref t) => {
                    match t {
                        &Token::Literal(_,_) | &Token::Pound | &Token::Comma => {},
                        _ => {
                            for l in self.get_line_indexes(*s) {
                                cover.insert(l);
                            }
                        },
                    }
                },
                _ => {},
            }
        }
        let pb = PathBuf::from(self.codemap.span_to_filename(s) as String);
        if let Ok(ts) = self.codemap.span_to_lines(s) {
            for l in ts.lines.iter().skip(1) {
                let linestr = if let Some(linestr) = ts.file.get_line(l.line_index) {
                    linestr
                } else {
                    ""
                };
                if !cover.contains(&l.line_index) && (linestr.len() <= (l.end_col.0 - l.start_col.0)) {
                    self.lines.push((pb.clone(), l.line_index+1));     
                }
            }
        }
    }
    
    /// Ignores where statements given the generics struct and the span this where
    /// is contained within. In every instance tested the first line of the containing
    /// span is coverable (as it is function definition) therefore shouldn't be 
    /// added to ignore list.
    fn ignore_where_statements(&mut self, gen: &Generics, container: Span) {
        let pb = PathBuf::from(self.codemap.span_to_filename(gen.span) as String);
        let first_line = {
            let mut line = None;
            if let Ok(fl) = self.codemap.span_to_lines(container) {
                if let Some(s) = fl.lines.get(0) {
                    line = Some(s.line_index);
                }
            } 
            line
        };
        if let Some(first_line) = first_line {
            for w in &gen.where_clause.predicates {
                let span = match w {
                    &WherePredicate::BoundPredicate(ref b) => b.span,
                    &WherePredicate::RegionPredicate(ref r) => r.span,
                    &WherePredicate::EqPredicate(ref e) => e.span,
                };
                let end = self.get_line_indexes(span.end_point());
                if let Some(&end) = end.last() {
                    for l in (first_line+1)..(end+1) {
                        self.lines.push((pb.clone(), l+1));
                    }
                }
            }
        }
    }
}


impl<'v, 'a> Visitor<'v> for CoverageVisitor<'a> {
 
    fn visit_item(&mut self, i: &'v Item) {
        match i.node {
            ItemKind::ExternCrate(..) => self.ignore_lines(i.span),
            ItemKind::Fn(_, _, _, _, ref gen, ref block) => {
                if attr::contains_name(&i.attrs, "test") && self.config.ignore_tests {
                    self.ignore_lines(i.span);
                    self.ignore_lines(block.deref().span);
                } else if attr::contains_name(&i.attrs, "inline") {
                    self.cover_lines(block.deref().span);
                }
                if attr::contains_name(&i.attrs, "ignore") && !self.config.run_ignored {
                    self.ignore_lines(i.span);
                    self.ignore_lines(block.deref().span);
                }
                self.ignore_where_statements(gen, i.span);
            },
            ItemKind::Impl(_, _, _, _, _, _, ref items) => {
                for i in items {
                    match i.node {
                        ImplItemKind::Method(ref sig,_) => {
                            self.cover_lines(i.span);
                            self.ignore_where_statements(&sig.generics, i.span);
                        }
                        _ => {},
                    }
                }
            },
            ItemKind::Use(_) => {
                self.ignore_lines(i.span);
            }
            _ => {},
        }
        visit::walk_item(self, i);
    }


    fn visit_mod(&mut self, m: &'v Mod, s: Span, _attrs: &[Attribute], _n: NodeId) {
        // If mod is cfg(test) and --ignore-tests ignore contents!
        if let Ok(fl) = self.codemap.span_to_lines(s) {
            if self.config.ignore_tests && self.contains_cfg_test(_attrs) {
                self.ignore_lines(s);
                if fl.lines.len() == 1 {
                    // Ignore the file
                    self.ignore_lines(m.inner);
                }
            }
            else { 
                if fl.lines.len() == 1 {
                    // mod imports show up as coverable. Ignore
                    self.ignore_lines(s);
                }
                let mod_path = PathBuf::from(self.codemap.span_to_filename(m.inner));
                if !self.covered.contains(&mod_path) {
                    visit::walk_mod(self, m);
                }
            }
        } 
    }
   

    fn visit_trait_item(&mut self, ti: &TraitItem) {
        match ti.node {
            TraitItemKind::Method(_, Some(ref b)) => {
                self.cover_lines(b.span);
            },
            _ => {},
        }
        visit::walk_trait_item(self, ti);
    }

    fn visit_fn(&mut self, fk: FnKind, fd: &FnDecl, s: Span, _: NodeId) {
        match fk {
            FnKind::ItemFn(_, g, _,_,_,_,_) => {
                if !g.ty_params.is_empty() {
                    self.cover_lines(s);
                }
            },
            FnKind::Method(_, sig, _, _) => {
                if !sig.generics.ty_params.is_empty() {
                    self.cover_lines(s);
                }
            },
            _ => {},
        }
        visit::walk_fn(self, fk, fd, s);
    }

    fn visit_expr(&mut self, ex: &Expr) {
        if let Ok(s) = self.codemap.span_to_lines(ex.span) {
            // If expression is multiple lines we might have to remove some of 
            // said lines.
            if s.lines.len() > 1 {
                let mut cover: HashSet<usize>  = HashSet::new();
                match ex.node {
                    ExprKind::Call(_, ref args) => {
                        cover.insert(s.lines[0].line_index);
                        for a in args {
                            match a.node {
                                ExprKind::Lit(..) => {},
                                _ => {
                                    for l in self.get_line_indexes(a.span) {
                                        cover.insert(l);
                                    }
                                },
                            }
                        }
                    },
                    ExprKind::MethodCall(_, _, ref args) => {
                        let mut it = args.iter();
                        it.next(); // First is function call
                        for i in it {
                            match i.node {
                                ExprKind::Lit(..) => {},
                                _ => {
                                    for l in self.get_line_indexes(i.span) {
                                        cover.insert(l);
                                    }
                                },
                            }
                        }
                    },
                    ExprKind::Mac(ref mac) => {
                        self.ignore_mac_args(&mac.node, ex.span);
                    },
                    ExprKind::Struct(_, ref fields, _) => {
                        for f in fields.iter() {
                            match f.expr.node {
                                ExprKind::Lit(_) => {
                                    self.ignore_lines(f.span);
                                }, 
                                _ => {},
                            }
                        }
                    },
                    _ => {},
                }
                if !cover.is_empty() {
                    let pb = PathBuf::from(self.codemap.span_to_filename(ex.span) as String);
                    for l in &s.lines {
                        if !cover.contains(&l.line_index) {
                            self.lines.push((pb.clone(), l.line_index + 1));
                        }
                    }
                }
            }
        }
        visit::walk_expr(self, ex);
    }

    fn visit_mac_def(&mut self, mac: &MacroDef, _id: NodeId) {
        // Makes sure the macro definitions have ignorable lines ignored as well.
        for token in mac.stream().into_trees() {
            match token {
                TokenTree::Token(span, _) => self.find_ignorable_lines(span),
                TokenTree::Delimited(span, _) => self.find_ignorable_lines(span),
            }
        }
    }

    fn visit_mac(&mut self, mac: &Mac) {
        // Use this to ignore unreachable lines
        let mac_text = &format!("{}", mac.node.path)[..];
        // TODO unimplemented should have extra logic to exclude the
        // function from coverage
        match mac_text {
            "unimplemented" | "unreachable" | "include" => self.ignore_lines(mac.span),
            _ => self.ignore_mac_args(&mac.node, mac.span),
        }
        visit::walk_mac(self, mac);
    }


    /// Ignores attributes which may get identified as coverable lines.
    fn visit_attribute(&mut self, attr: &Attribute) {
        if attr.check_name("derive") {
            self.ignore_lines(attr.span);
        }
    }

    
    /// Struct fields are mistakenly identified as instructions and uncoverable.
    fn visit_struct_field(&mut self, s: &'v StructField) {
        self.ignore_lines(s.span);
        visit::walk_struct_field(self, s);
    }


    fn visit_block(&mut self, b: &'v Block) {
        self.find_ignorable_lines(b.span);
        visit::walk_block(self, b);
    }


    fn visit_stmt(&mut self, s: &Stmt) {
        match s.node {
            StmtKind::Mac(ref p) => {
                let ref mac = p.0.node;
                self.ignore_mac_args(mac, s.span);
            },
            _ => {}
        }
        visit::walk_stmt(self, s);
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use syntex_syntax::parse::filemap_to_parser;
    use syntex_syntax::parse::parser::Parser;

    struct TestContext {
        conf: Config,
        codemap: Rc<CodeMap>,
        parse_session: ParseSess
    }

    impl TestContext {
        fn generate_parser(&self, filename: &str, src_string: &str) -> Parser {
            let filemap = self.codemap.new_filemap(filename.to_string(), 
                                                   src_string.to_string());
            filemap_to_parser(&self.parse_session, filemap)
        }
    }

    impl Default for TestContext {
        fn default() -> TestContext {
            let codemap = Rc::new(CodeMap::new(FilePathMapping::empty()));
            let handler = Handler::with_tty_emitter(ColorConfig::Auto, false, false, Some(codemap.clone()));
            let parse_session = ParseSess::with_span_handler(handler, codemap.clone());
            TestContext {
                conf: Config::default(),
                codemap: codemap,
                parse_session: parse_session
            }
        }
    }

    fn parse_crate(ctx: &TestContext, parser: &mut Parser) -> Vec<usize>  {
        let krate = parser.parse_crate_mod();
        assert!(krate.is_ok());
        let krate = krate.unwrap();
        let unused: HashSet<PathBuf> = HashSet::new();
        let mut visitor = CoverageVisitor::from_session(&ctx.parse_session, &unused, &ctx.conf);
        visitor.visit_mod(&krate.module, krate.span, &krate.attrs, NodeId::new(0));
        visitor.lines.iter().map(|x| x.1).collect::<Vec<_>>()
    }
    
    #[test] 
    fn filter_str_literals() {
        let ctx = TestContext::default();
        let mut parser = ctx.generate_parser("literals.rs", "fn test() {\nwriteln!(#\"test\n\ttest\n\ttest\"#);\n}\n");
        let lines = parse_crate(&ctx, &mut parser);
        assert!(lines.len() > 1);
        assert!(lines.contains(&3));
        assert!(lines.contains(&4));
        
        let ctx = TestContext::default();
        let mut parser = ctx.generate_parser("literals.rs", "fn test() {\nwrite(\"test\ntest\ntest\");\n}\nfn write(s:&str){}");
        let lines = parse_crate(&ctx, &mut parser);
        assert!(lines.len() > 1);
        assert!(lines.contains(&3));
        assert!(lines.contains(&4));
        
        let ctx = TestContext::default();
        let mut parser = ctx.generate_parser("literals.rs", "\n\nfn test() {\nwriteln!(\n#\"test\"#\n);\n}\n");
        let lines = parse_crate(&ctx, &mut parser);
        assert!(lines.contains(&5));
    }

    #[test]
    fn filter_struct_members() {
        let ctx = TestContext::default();
        let mut parser = ctx.generate_parser("struct_test.rs", "#[derive(Debug)]\npub struct Struct {\npub i: i32,\nj:String,\n}");
        let lines = parse_crate(&ctx, &mut parser);
        
        assert_eq!(lines.len(), 3);
        assert!(lines.contains(&1)); 
        assert!(lines.contains(&3)); 
        assert!(lines.contains(&4)); 
    }

    #[test]
    fn filter_mods() {
        let ctx = TestContext::default();
        let mut parser = ctx.generate_parser("test.rs", "mod foo {\nfn double(x:i32)->i32 {\n x*2\n}\n}");
        let lines = parse_crate(&ctx, &mut parser);
        assert!(!lines.contains(&3));
        
        let mut parser = ctx.generate_parser("test.rs", "mod foo{}");
        let lines = parse_crate(&ctx, &mut parser);
        assert!(lines.contains(&1));
    }

    #[test]
    fn filter_macros() {
        let ctx = TestContext::default();
        let mut parser = ctx.generate_parser("test.rs", "\n\nfn unused() {\nunimplemented!();\n}"); 
        
        let lines = parse_crate(&ctx, &mut parser);
        // Braces should be ignored so number could be higher
        assert!(lines.len() >= 1);
        assert!(lines.contains(&4));
        
        let mut parser = ctx.generate_parser("test.rs", "fn unused() {\nunreachable!();\n}"); 
        let lines = parse_crate(&ctx, &mut parser);
        assert!(lines.len() >= 1);
        assert!(lines.contains(&2));
        
        let mut parser = ctx.generate_parser("test.rs", "fn unused() {\nprintln!(\"text\");\n}"); 
        let lines = parse_crate(&ctx, &mut parser);
        assert!(!lines.contains(&2));
    }
   
    #[test]
    fn filter_tests() {
        let ctx = TestContext::default();
        let src_string = "#[cfg(test)]\nmod tests {\n fn boo(){\nassert!(true);\n}\n}";
        let mut parser = ctx.generate_parser("test.rs", src_string);
        let lines = parse_crate(&ctx, &mut parser);
        assert!(!lines.contains(&4));

        let mut ctx = TestContext::default();
        ctx.conf.ignore_tests = true;
        let mut parser = ctx.generate_parser("test.rs", src_string);
        let lines = parse_crate(&ctx, &mut parser);
        assert!(lines.contains(&4));

        let ctx = TestContext::default();
        let src_string = "#[test]\nfn mytest() { \n assert!(true);\n}";
        let mut parser = ctx.generate_parser("test.rs", src_string);
        let lines = parse_crate(&ctx, &mut parser);
        assert!(!lines.contains(&2));
        assert!(!lines.contains(&3));

        let mut ctx = TestContext::default();
        ctx.conf.ignore_tests = true;
        let mut parser = ctx.generate_parser("test.rs", src_string);
        let lines = parse_crate(&ctx, &mut parser);
        assert!(lines.contains(&2));
        assert!(lines.contains(&3));

    }
}
