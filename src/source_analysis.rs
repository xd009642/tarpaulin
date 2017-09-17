use cargo::core::{Workspace, Source};
use cargo::sources::PathSource;
use syntex_syntax::visit::{Visitor, FnKind};
use syntex_syntax::codemap::{CodeMap, Span, BytePos, FilePathMapping};
use syntex_syntax::ast::{NodeId, Mac, Attribute, MetaItemKind, Stmt, FnDecl};
use syntex_syntax::parse::{self, ParseSess};
use syntex_syntax::errors::Handler;
use syntex_syntax::errors::emitter::ColorConfig;
use std::path::{PathBuf, Path};
use std::ffi::OsStr;
use std::rc::Rc;
use config::Config;

struct IgnoredLines<'a> {
    lines: Vec<(PathBuf, usize)>,
    codemap: &'a CodeMap,
    parse_session: &'a ParseSess,
}
/*
 *  MetaItem contains #[test] etc use it to filter those lines and test functions!
 *
 * Need to use walk to traverse DEEPER. is fn under attr?
 */

pub fn get_lines_to_ignore(project: &Workspace) -> Vec<(PathBuf, usize)> {
    let mut result: Vec<(PathBuf, usize)> = Vec::new();
    
    let pkg = project.current().unwrap();
    let mut src = PathSource::new(pkg.root(), pkg.package_id().source_id(), project.config());
    src.update();

    let codemap = Rc::new(CodeMap::new(FilePathMapping::empty()));
    let handler = Handler::with_tty_emitter(ColorConfig::Auto, false, false, Some(codemap.clone()));
    let mut parse_session = ParseSess::with_span_handler(handler, codemap.clone());
    
    for file in src.list_files(&pkg).unwrap().iter() {
        // Do I need to do this from the root? 
        if file.extension() == Some(OsStr::new("rs")) {
            // Rust source file
            let mut parser = parse::new_parser_from_file(&parse_session, &file);
            if let Ok(mut krate) = parser.parse_crate_mod() {
                let mut visitor = IgnoredLines::from_session(&parse_session);
                visitor.visit_mod(&krate.module, krate.span, &krate.attrs, NodeId::new(0));
                result.append(&mut visitor.lines);
            }
        }
    }
    result
}

impl<'a> IgnoredLines<'a> {
    /// Construct a new ignored lines object for the given project
    fn from_session(session: &'a ParseSess) -> IgnoredLines<'a> {
        IgnoredLines {
            lines: vec![],
            codemap: session.codemap(),
            parse_session: &session
        }
    }
}


impl<'v, 'a> Visitor<'v> for IgnoredLines<'a> {
   
    fn visit_fn(&mut self, fk: FnKind<'v>, fd: &'v FnDecl, s: Span, _: NodeId) {
    
    }

    fn visit_stmt(&mut self, s: &Stmt) {
        unimplemented!();
    }

    fn visit_mac(&mut self, mac: &Mac) {
        // Use this to ignore unreachable lines
        unimplemented!();
    }

    fn visit_attribute(&mut self, attr: &Attribute) {
        if attr.is_word() {
            // Ignore the line. If test and ignoring tests ignore
        }
    }
    
}

