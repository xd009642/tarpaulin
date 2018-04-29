use std::path::{PathBuf, Path};
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::io::{BufReader, BufRead};
use cargo::core::{Workspace, Package};
use cargo::sources::PathSource;
use cargo::util::Config as CargoConfig;
use syn::{parse_file, Item, ItemMod, ItemFn, Ident, Meta, NestedMeta, Stmt};
use proc_macro2::Span;
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
    pub fn cover_span(&mut self, span: &Span) {
        for i in span.start().line..(span.end().line +1) {
            if !self.ignore.contains(&i) {
                self.cover.insert(i);
            }
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
                    find_ignorable_lines(&content, &mut analysis);
                    process_items(&file.items, config, &mut analysis);
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


fn process_items(items: &[Item], config: &Config, analysis: &mut LineAnalysis) {
    for item in items {
        match item {
            Item::ExternCrate(i) => analysis.ignore_span(&i.extern_token.0),
            Item::Use(i) => analysis.ignore_span(&i.use_token.0),
            Item::Mod(i) => visit_mod(i, analysis, config),
            Item::Fn(i) => visit_fn(i, analysis, config),
            _ =>{}
        } 
    }
}


fn process_statements(stmts: &[Stmt], config: &Config, analysis: &mut LineAnalysis) {
    for stmt in stmts {
        match stmt {
            Stmt::Item(i) => process_items(&[i.clone()], config, analysis),
            _ => {},
        }
    }
}


fn visit_mod(module: &ItemMod, analysis: &mut LineAnalysis, config: &Config) {
    analysis.ignore_span(&module.mod_token.0); 
    let mut check_insides = true;
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
    if check_insides {
        if let Some((_, ref items)) = module.content {
            process_items(items, config, analysis);
        }
    }
}


fn visit_fn(func: &ItemFn, analysis: &mut LineAnalysis, config: &Config) {
    // Need to read the nested meta.. But this should work for fns
    let mut ignore_func = false;
    for attr in &func.attrs {
        if let Some(x) = attr.interpret_meta() {
            let id = x.name();
            if id == Ident::from("test") {
                ignore_func = true;
            } else if id == Ident::from("derive") {
                analysis.ignore_span(&attr.bracket_token.0);
            }
        }
    }
    if ignore_func && config.ignore_tests {
        analysis.ignore_span(&func.decl.fn_token.0);
        analysis.ignore_span(&func.block.brace_token.0);
    } else {
        process_statements(&func.block.stmts, config, analysis);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_file;

    #[test] 
    fn filter_str_literals() {
        let mut lines = LineAnalysis::new();
        let config = Config::default();
        let parser = parse_file("fn test() {\nwriteln!(#\"test\n\ttest\n\ttest\"#);\n}\n").unwrap();
        process_items(&parser.items, &config, &mut lines);
        assert!(lines.ignore.len() > 1);
        assert!(lines.ignore.contains(&3));
        assert!(lines.ignore.contains(&4));
        
        let parser = parse_file("fn test() {\nwrite(\"test\ntest\ntest\");\n}\nfn write(s:&str){}").unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &config, &mut lines);
        assert!(lines.ignore.len() > 1);
        assert!(lines.ignore.contains(&3));
        assert!(lines.ignore.contains(&4));
        
        let mut lines = LineAnalysis::new();
        let parser = parse_file("\n\nfn test() {\nwriteln!(\n#\"test\"#\n);\n}\n").unwrap();
        process_items(&parser.items, &config, &mut lines);
        assert!(lines.ignore.contains(&5));
    }

    #[test]
    fn filter_struct_members() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let parser = parse_file("#[derive(Debug)]\npub struct Struct {\npub i: i32,\nj:String,\n}").unwrap();
        process_items(&parser.items, &config, &mut lines);
        
        assert_eq!(lines.ignore.len(), 3);
        assert!(lines.ignore.contains(&1)); 
        assert!(lines.ignore.contains(&3)); 
        assert!(lines.ignore.contains(&4)); 
    }

    #[test]
    fn filter_struct_consts() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let parser = parse_file("struct T{x:String, y:i32}\nfn test()-> T {\nT{\nx:\"hello\".to_string(),\ny:4,\n}\n}").unwrap();
        process_items(&parser.items, &config, &mut lines);
        
        assert!(lines.ignore.contains(&5));
    }

    #[test]
    fn filter_mods() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let parser = parse_file("mod foo {\nfn double(x:i32)->i32 {\n x*2\n}\n}").unwrap();
        process_items(&parser.items, &config, &mut lines);
        assert!(!lines.ignore.contains(&3));
        
        let mut lines = LineAnalysis::new();
        let parser = parse_file("mod foo{}").unwrap();
        process_items(&parser.items, &config, &mut lines);
        assert!(lines.ignore.contains(&1));
    }

    #[test]
    fn filter_macros() {
        let config = Config::default();
        let mut lines = LineAnalysis::new();
        let parser = parse_file("\n\nfn unused() {\nunimplemented!();\n}").unwrap(); 
        process_items(&parser.items, &config, &mut lines);
        
        // Braces should be ignored so number could be higher
        assert!(lines.ignore.len() >= 1);
        assert!(lines.ignore.contains(&4));
        
        let mut lines = LineAnalysis::new();
        let parser = parse_file("fn unused() {\nunreachable!();\n}").unwrap(); 
        process_items(&parser.items, &config, &mut lines);
        assert!(lines.ignore.len() >= 1);
        assert!(lines.ignore.contains(&2));
        
        let mut lines = LineAnalysis::new();
        let parser = parse_file("fn unused() {\nprintln!(\"text\");\n}").unwrap(); 
        process_items(&parser.items, &config, &mut lines);
        assert!(!lines.ignore.contains(&2));
    }
   
    #[test]
    fn filter_tests() {
        let mut config = Config::default();
        let mut lines = LineAnalysis::new();
        let src_string = "#[cfg(test)]\nmod tests {\n fn boo(){\nassert!(true);\n}\n}";
        let parser = parse_file(src_string).unwrap();
        process_items(&parser.items, &config, &mut lines);
        assert!(!lines.ignore.contains(&4));

        config.ignore_tests = true;
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &config, &mut lines);
        assert!(lines.ignore.contains(&4));

        let src_string = "#[test]\nfn mytest() { \n assert!(true);\n}";
        let parser = parse_file(src_string).unwrap();
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &config, &mut lines);
        assert!(!lines.ignore.contains(&2));
        assert!(!lines.ignore.contains(&3));

        config.ignore_tests = true;
        let mut lines = LineAnalysis::new();
        process_items(&parser.items, &config, &mut lines);
        assert!(lines.ignore.contains(&2));
        assert!(lines.ignore.contains(&3));

    }
}
