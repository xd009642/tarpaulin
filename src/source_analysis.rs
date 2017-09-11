
use syntex_syntax::visit::Visitor;
use syntex_syntax::codemap::{CodeMap, Span, BytePos, FilePathMapping};
use syntex_syntax::ast::{NodeId, Mac, Attribute, MetaItemKind, Stmt};
use syntex_syntax::parse::{self, ParseSess};
use syntex_syntax::errors::Handler;
use syntex_syntax::errors::emitter::ColorConfig;
use std::path::{PathBuf, Path};
use std::rc::Rc;
use config::Config;

pub struct IgnoredLines<'a> {
    pub lines: Vec<(PathBuf, usize)>,
    codemap: &'a CodeMap,
    config: &'a Config,
    parse_session: &'a ParseSess,
}
/*
 *  MetaItem contains #[test] etc use it to filter those lines and test functions!
 *
 * Need to use walk to traverse DEEPER. is fn under attr?
 */

pub fn get_lines_to_ignore(config: &Config) -> Vec<(PathBuf, usize)> {
        let codemap = Rc::new(CodeMap::new(FilePathMapping::empty()));
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, config.verbose, false, Some(codemap.clone()));
        let mut parse_session = ParseSess::with_span_handler(handler, codemap.clone());

        let lines = IgnoredLines::from_session(&parse_session, config);
        // Add files to codemap
        // Create AST
        // Visit nodes
        // lines.visit_mod()
        lines.lines
}

impl<'a> IgnoredLines<'a> {
    /// Construct a new ignored lines object for the given project
    fn from_session(session: &'a ParseSess, config: &'a Config) -> IgnoredLines<'a> {
        IgnoredLines {
            lines: vec![],
            codemap: session.codemap(),
            config: config,
            parse_session: &session
        }
    }

    fn parse_project(&self) {
        unimplemented!();
    }
}


impl<'v, 'a> Visitor<'v> for IgnoredLines<'a> {
    
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

