
use syntex_syntax::visit::Visitor;
use syntex_syntax::codemap::{CodeMap, Span, BytePos, FilePathMapping};
use syntex_syntax::ast::{NodeId, Mac, Attribute, MetaItemKind, Stmt};
use syntex_syntax::parse::(self, ParseSess};
use std::path::{PathBuf, Path};

use config::Config;

pub struct IgnoredLines<'a> {
    pub lines: Vec<(PathBuf, usize)>,
    codemap: &'a CodeMap,
    config: &Config,
    parse_session: &ParseSess,
}
/*
 *  MetaItem contains #[test] etc use it to filter those lines and test functions!
 *
 * Need to use walk to traverse DEEPER. is fn under attr?
 */

impl<'a> IgnoredLines<'a> {
    /// Construct a new ignored lines object for the given project
    fn new(config: &Config) -> Option<IgnoredLines<'a>> {
        let codemap = CodeMap::new(FilePathMapping::empty());
        None
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

