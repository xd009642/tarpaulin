use std::rc::Rc;
use std::ops::Deref;
use std::path::PathBuf;
use std::collections::HashSet;
use cargo::core::Workspace;
use cargo::sources::PathSource;
use syntex_syntax::attr;
use syntex_syntax::visit::{self, Visitor, FnKind};
use syntex_syntax::codemap::{CodeMap, Span, FilePathMapping};
use syntex_syntax::ast::{NodeId, Mac, Attribute, Stmt, StmtKind, FnDecl, Mod, 
    StructField, Block, Item, ItemKind};
use syntex_syntax::parse::{self, ParseSess};
use syntex_syntax::errors::Handler;
use syntex_syntax::errors::emitter::ColorConfig;
use syntex_syntax::ext::expand::MacroExpander;
use config::Config;

struct IgnoredLines<'a> {
    lines: Vec<(PathBuf, usize)>,
    covered: &'a HashSet<PathBuf>,
    codemap: &'a CodeMap,
    config: &'a Config, 
}


/// Returns a list of files and line numbers to ignore (not indexes!)
pub fn get_lines_to_ignore(project: &Workspace, config: &Config) -> Vec<(PathBuf, usize)> {
    let mut result: Vec<(PathBuf, usize)> = Vec::new();
    
    let pkg = project.current().unwrap();
    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), project.config());
    if let Ok(package) = src.root_package() {

        let codemap = Rc::new(CodeMap::new(FilePathMapping::empty()));
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, false, false, Some(codemap.clone()));
        let parse_session = ParseSess::with_span_handler(handler, codemap.clone());
        
        let mut done_files: HashSet<PathBuf> = HashSet::new();
        for target in package.targets() {
            let file = target.src_path();
            if !(config.ignore_tests && file.starts_with(project.root().join("tests"))) {
                let mut parser = parse::new_parser_from_file(&parse_session, file);
                if let Ok(krate) = parser.parse_crate_mod() {
                    
                    let mut lines = {
                        let mut visitor = IgnoredLines::from_session(&parse_session, &done_files, config);
                        visitor.visit_mod(&krate.module, krate.span, &krate.attrs, NodeId::new(0));
                        visitor.lines
                    };

                    for l in &lines {
                        done_files.insert(l.0.clone());
                    }
                    result.append(&mut lines);
                }
            }
        }
    }
    result
}

impl<'a> IgnoredLines<'a> {
    /// Construct a new ignored lines object for the given project
    fn from_session(session: &'a ParseSess, 
                    covered: &'a HashSet<PathBuf>, 
                    config: &'a Config) -> IgnoredLines<'a> {
        IgnoredLines {
            lines: vec![],
            covered: covered,
            codemap: session.codemap(),
            config: config,
        }
    }
    
    /// Add lines to the line ignore list
    fn ignore_lines(&mut self, span: Span) {
        if let Ok(s) = self.codemap.span_to_lines(span) {
            for line in &s.lines {
                let pb = PathBuf::from(self.codemap.span_to_filename(span) as String);
                // Line number is index+1
                self.lines.push((pb, line.line_index + 1));
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
                    // Is this one of those pointless {, } or }; only lines?
                    if !s.chars().any(|x| !"{}[];\t ,".contains(x)) {
                        self.lines.push((pb, line.line_index + 1));
                    }
                }
            }
        }
    }
}


impl<'v, 'a> Visitor<'v> for IgnoredLines<'a> {
 
    fn visit_item(&mut self, i: &'v Item) {
        match i.node {
            ItemKind::ExternCrate(..) => self.ignore_lines(i.span),
            ItemKind::Fn(_, _, _, _, _, ref block) => {
                if attr::contains_name(&i.attrs, "test") && self.config.ignore_tests {
                    self.ignore_lines(i.span);
                    self.ignore_lines(block.deref().span);
                }
            },
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


    fn visit_fn(&mut self, fk: FnKind<'v>, fd: &'v FnDecl, s: Span, _: NodeId) {
        visit::walk_fn(self, fk, fd, s);
    }


    fn visit_mac(&mut self, mac: &Mac) {
        // Use this to ignore unreachable lines
        let mac_text = &format!("{}", mac.node.path)[..];
        // TODO unimplemented should have extra logic to exclude the
        // function from coverage
        match mac_text {
            "unimplemented" => self.ignore_lines(mac.span),
            "unreachable" => self.ignore_lines(mac.span),
            _ => {},
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
        let mut visitor = IgnoredLines::from_session(&ctx.parse_session, &unused, &ctx.conf);
        visitor.visit_mod(&krate.module, krate.span, &krate.attrs, NodeId::new(0));
        visitor.lines.iter().map(|x| x.1).collect::<Vec<_>>()
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
