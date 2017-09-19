use cargo::core::{Workspace, Source};
use cargo::sources::PathSource;
use syntex_syntax::visit::{self, Visitor, FnKind};
use syntex_syntax::codemap::{CodeMap, Span, FilePathMapping};
use syntex_syntax::ast::{NodeId, Mac, Attribute, Stmt, StmtKind, FnDecl, Mod};
use syntex_syntax::parse::{self, ParseSess};
use syntex_syntax::errors::Handler;
use syntex_syntax::errors::emitter::ColorConfig;
use std::path::PathBuf;
use std::ffi::OsStr;
use std::rc::Rc;
use config::Config;

struct IgnoredLines<'a> {
    lines: Vec<usize>,
    codemap: &'a CodeMap,
    parse_session: &'a ParseSess,
    config: &'a Config, 
    started: bool,
}
/*
 *  MetaItem contains #[test] etc use it to filter those lines and test functions!
 *
 * Need to use walk to traverse DEEPER. is fn under attr?
 */

pub fn get_lines_to_ignore(project: &Workspace, config: &Config) -> Vec<(PathBuf, usize)> {
    let mut result: Vec<(PathBuf, usize)> = Vec::new();
    
    let pkg = project.current().unwrap();
    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), project.config());
    src.update();

    let codemap = Rc::new(CodeMap::new(FilePathMapping::empty()));
    let handler = Handler::with_tty_emitter(ColorConfig::Auto, false, false, Some(codemap.clone()));
    let parse_session = ParseSess::with_span_handler(handler, codemap.clone());
    
    for file in src.list_files(&pkg).unwrap().iter() {
        if file.extension() == Some(OsStr::new("rs")) {
            // Rust source file
            println!("Parsing {}", file.display());
            let mut parser = parse::new_parser_from_file(&parse_session, &file);
            if let Ok(krate) = parser.parse_crate_mod() {
                let mut visitor = IgnoredLines::from_session(&parse_session, config);
                visitor.visit_mod(&krate.module, krate.span, &krate.attrs, NodeId::new(0));
                //result.append(&mut visitor.lines.map(|x);
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
            parse_session: &session,
            config: config,
            started: false
        }
    }
    
    fn ignore_lines(&mut self, span: Span) {
        if let Ok(s) = self.codemap.span_to_lines(span) {
            for line in &s.lines {
                self.lines.push(line.line_index);
            }
        }
    }    

}

impl<'v, 'a> Visitor<'v> for IgnoredLines<'a> {
  
    fn visit_mod(&mut self, m: &'v Mod, _s: Span, _attrs: &[Attribute], _n: NodeId) {
        // We want to limit ourselves to only this source file.
        // This avoids repeated hits and issues where there are multiple compilation targets
        if self.started == false {
            self.started = true;
            visit::walk_mod(self, m);
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
    }

    fn visit_attribute(&mut self, attr: &Attribute) {
        if attr.check_name("test") && self.config.ignore_tests {
            self.ignore_lines(attr.span);
        } else if attr.check_name("derive") {
            self.ignore_lines(attr.span);
        }
    }

}

