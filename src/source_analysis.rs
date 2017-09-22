use cargo::core::{Workspace, Source};
use cargo::sources::PathSource;
use syntex_syntax::attr;
use syntex_syntax::visit::{self, Visitor, FnKind};
use syntex_syntax::codemap::{CodeMap, Span, FilePathMapping};
use syntex_syntax::ast::{NodeId, Mac, Attribute, Stmt, StmtKind, FnDecl, Mod, 
    StructField, Block, Item, ItemKind};
use syntex_syntax::parse::{self, ParseSess};
use syntex_syntax::errors::Handler;
use syntex_syntax::errors::emitter::ColorConfig;
use std::path::PathBuf;
use std::ffi::OsStr;
use std::rc::Rc;
use std::ops::Deref;
use config::Config;

struct IgnoredLines<'a> {
    lines: Vec<usize>,
    codemap: &'a CodeMap,
    config: &'a Config, 
    started: bool,
}


/// Returns a list of files and line numbers to ignore (not indexes!)
pub fn get_lines_to_ignore(project: &Workspace, config: &Config) -> Vec<(PathBuf, usize)> {
    let mut result: Vec<(PathBuf, usize)> = Vec::new();
    
    let pkg = project.current().unwrap();
    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), project.config());
    // If this fails we should just iterate over no files. No need to care.
    let _ = src.update();

    let codemap = Rc::new(CodeMap::new(FilePathMapping::empty()));
    let handler = Handler::with_tty_emitter(ColorConfig::Auto, false, false, Some(codemap.clone()));
    let parse_session = ParseSess::with_span_handler(handler, codemap.clone());
    
    for file in src.list_files(&pkg).unwrap().iter() {
        if !(config.ignore_tests && file.starts_with(project.root().join("tests"))) {
            if file.extension() == Some(OsStr::new("rs")) {
                // Rust source file
                let mut parser = parse::new_parser_from_file(&parse_session, &file);
                if let Ok(krate) = parser.parse_crate_mod() {
                    let mut visitor = IgnoredLines::from_session(&parse_session, config);
                    visitor.visit_mod(&krate.module, krate.span, &krate.attrs, NodeId::new(0));
                    result.append(&mut visitor.lines.iter()
                                                    .map(|x| (file.to_path_buf(), *x))
                                                    .collect::<Vec<_>>());
                }
            }
        }
    }
    result
}

impl<'a> IgnoredLines<'a> {
    /// Construct a new ignored lines object for the given project
    fn from_session(session: &'a ParseSess, config: &'a Config) -> IgnoredLines<'a> {
        IgnoredLines {
            lines: vec![],
            codemap: session.codemap(),
            config: config,
            started: false
        }
    }
    
    /// Add lines to the line ignore list
    fn ignore_lines(&mut self, span: Span) {
        if let Ok(s) = self.codemap.span_to_lines(span) {
            for line in &s.lines {
                // Line number is index+1
                self.lines.push(line.line_index + 1);
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
                if let Some(s) = l.file.get_line(line.line_index) {
                    // Is this one of those pointless {, } or }; only lines?
                    if !s.chars().any(|x| !"{}[];\t ".contains(x)) {
                        self.lines.push(line.line_index + 1);
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
        // We want to limit ourselves to only this source file.
        // This avoids repeated hits and issues where there are multiple compilation targets
        if self.started == false {
            self.started = true;
            visit::walk_mod(self, m);
        } else {
            // Needs to be more advanced - cope with mod { .. } in file.
            // Also if mod is cfg(test) and --ignore-tests ignore contents!
            if let Ok(fl) = self.codemap.span_to_lines(s) {
                if fl.lines.len() > 1 {
                    if self.config.ignore_tests && self.contains_cfg_test(_attrs) {
                        self.ignore_lines(s);
                    }
                    visit::walk_mod(self, m);
                } else {
                    // one line mod, so either referencing another file or something
                    // not worth covering.
                    self.ignore_lines(s);
                }
            }
        }
    }


    fn visit_fn(&mut self, fk: FnKind<'v>, fd: &'v FnDecl, s: Span, _: NodeId) {
        visit::walk_fn(self, fk, fd, s);
    }


    fn visit_stmt(&mut self, s: &Stmt) {
        match s.node {
            StmtKind::Mac(_) => visit::walk_stmt(self, s),
            _ => {},
        }
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

