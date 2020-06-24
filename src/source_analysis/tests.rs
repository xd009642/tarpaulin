use crate::source_analysis::prelude::*;
use syn::parse_file;

#[test]
fn logical_lines_let_bindings() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn foo() {
            let x 
                  =
                    5;
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert_eq!(lines.logical_lines.get(&3).copied(), Some(2));
    assert_eq!(lines.logical_lines.get(&4).copied(), Some(2));

    let ctx = Context {
        config: &config,
        file_contents: "fn foo() {
        let x = (0..15).iter()
            .filter(|x| {
                if x % 3 == 0 {
                    true
                } else {
                    false
                }
            })
            .cloned()
            .collect::<Vec<u32>>();
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };

    let mut lines = LineAnalysis::new();
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    println!("Lines {:?}", lines);
    assert!(!lines.logical_lines.contains_key(&4));
    assert!(!lines.logical_lines.contains_key(&5));
    assert!(!lines.logical_lines.contains_key(&6));
    assert!(!lines.logical_lines.contains_key(&7));
    assert!(!lines.logical_lines.contains_key(&8));
    assert!(!lines.logical_lines.contains_key(&9));
    assert!(!lines.logical_lines.contains_key(&10));
    assert!(!lines.logical_lines.contains_key(&11));
}

#[test]
fn line_analysis_works() {
    let mut la = LineAnalysis::new();
    assert!(!la.should_ignore(0));
    assert!(!la.should_ignore(10));

    la.add_to_ignore(&[3, 4, 10]);
    assert!(la.should_ignore(3));
    assert!(la.should_ignore(4));
    assert!(la.should_ignore(10));
    assert!(!la.should_ignore(1));
}

#[test]
fn filter_str_literals() {
    let mut lines = LineAnalysis::new();
    let config = Config::default();
    let ctx = Context {
        config: &config,
        file_contents: "fn test() {
            writeln!(#\"test
                     \ttest
                     \ttest\"#);
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.len() > 1);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));

    let ctx = Context {
        config: &config,
        file_contents: "fn test() {
            write(\"test
                  test
                  test\");
        }
        fn write(s:&str){}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    let mut lines = LineAnalysis::new();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.len() > 1);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "

            fn test() {
                writeln!(
                    #\"test\"#
                    );
            }
        ",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(5)));
}

#[test]
fn filter_struct_members() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[derive(Debug)]\npub struct Struct {\npub i: i32,\nj:String,\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);

    assert!(lines.ignore.len() > 3);
    assert!(lines.ignore.contains(&Lines::Line(1)));
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));

    let ctx = Context {
        config: &config,
        file_contents: "#[derive(Debug)]\npub struct Struct (\n i32\n);",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);

    assert!(!lines.ignore.is_empty());
    assert!(lines.ignore.contains(&Lines::Line(3)));
}

#[test]
fn filter_enum_members() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[derive(Debug)]\npub enum E {\nI1,\nI2(u32),\nI3{\nx:u32,\n},\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);

    assert!(lines.ignore.len() > 3);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(5)));
    assert!(lines.ignore.contains(&Lines::Line(6)));
    assert!(lines.ignore.contains(&Lines::Line(7)));
}

#[test]
fn filter_struct_consts() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "struct T{x:String, y:i32}
            fn test()-> T {
                T{
                    x:String::from(\"hello\"), //function call should be covered
                    y:4,
                }
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(5)));
}

#[test]
fn filter_unreachable_unchecked() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn test() {
                core::hint::unreachable_unchecked();
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
}

#[test]
fn filter_loop_attr() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn test() {
                #[allow(clippy::option_unwrap_used)]
                loop {
                }
                #[allow(clippy::option_unwrap_used)]
                for i in 0..10 {
                }
                #[allow(clippy::option_unwrap_used)]
                while true {
                }
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
    assert!(lines.ignore.contains(&Lines::Line(5)));
    assert!(lines.ignore.contains(&Lines::Line(8)));
}

#[test]
fn filter_mods() {
    let config = Config::default();
    let ctx = Context {
        config: &config,
        file_contents: "mod foo {\nfn double(x:i32)->i32 {\n x*2\n}\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    let mut lines = LineAnalysis::new();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(3)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "mod foo;",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(1)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "mod foo{}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(1)));
}

#[test]
fn filter_macros() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "\n\nfn unused() {\nunimplemented!();\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);

    // Braces should be ignored so number could be higher
    assert!(!lines.ignore.is_empty());
    assert!(lines.ignore.contains(&Lines::Line(4)));
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "\n\nfn unused() {\nunreachable!();\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.is_empty());
    assert!(lines.ignore.contains(&Lines::Line(4)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn unreachable_match(x: u32) -> u32 {
            match x {
                1 => 5,
                2 => 7,
                _ => unreachable!(),
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(5)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn unused() {\nprintln!(\"text\");\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(2)));
}

#[test]
fn filter_tests() {
    let config = Config::default();
    let mut igconfig = Config::default();
    igconfig.ignore_tests = true;

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[cfg(test)]
            mod tests {
                fn boo(){
                    assert!(true);
                }\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(4)));

    let ctx = Context {
        config: &igconfig,
        file_contents: "#[cfg(test)]
            mod tests {
                fn boo(){
                    assert!(true);
                }
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };

    let mut lines = LineAnalysis::new();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(4)));

    let ctx = Context {
        config: &config,
        file_contents: "#[test]\nfn mytest() { \n assert!(true);\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    let mut lines = LineAnalysis::new();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(2)));
    assert!(!lines.ignore.contains(&Lines::Line(3)));

    let ctx = Context {
        config: &igconfig,
        file_contents: "#[test]\nfn mytest() { \n assert!(true);\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let mut lines = LineAnalysis::new();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
    assert!(lines.ignore.contains(&Lines::Line(3)));
}

#[test]
fn filter_test_utilities() {
    let mut config = Config::default();
    config.ignore_tests = true;

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "trait Thing {
            #[cfg(test)]
            fn boo(){
                assert!(true);
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));

    let config = Config::default();

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "trait Thing {
            #[cfg(test)]
            fn boo(){
                assert!(true);
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(3)));
    assert!(!lines.ignore.contains(&Lines::Line(4)));
}

#[test]
fn filter_where() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn boop<T>() -> T  where T:Default {
            T::default()
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(1)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn boop<T>() -> T
            where T:Default {
                T::default()
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "trait foof {
            fn boop<T>() -> T
            where T:Default {
                T::default()
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(3)));
}

#[test]
fn filter_derives() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[derive(Debug)]\nstruct T;",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(1)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "\n#[derive(Copy, Eq)]\nunion x { x:i32, y:f32}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
}

#[test]
fn filter_unsafe() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn unsafe_fn() {\n let x=1;\nunsafe {\nprintln!(\"{}\", x);\n}\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(!lines.ignore.contains(&Lines::Line(4)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn unsafe_fn() {\n let x=1;\nunsafe {println!(\"{}\", x);}\n}",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(3)));
}

#[test]
fn cover_generic_impl_methods() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "struct GenericStruct<T>(T);
        impl<T> GenericStruct<T> {
            fn hw(&self) {
                println!(\"hello world\");
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.cover.contains(&3));
    assert!(lines.cover.contains(&4));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "struct GenericStruct<T>{v:Vec<T>}
        impl<T> Default for GenericStruct<T> {
            fn default() -> Self {
                T {
                    v: vec![],
                }
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.cover.contains(&5));
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
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
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
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(15)));
    assert!(!lines.ignore.contains(&Lines::Line(19)));
}

#[test]
fn filter_use_statements() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "use std::collections::HashMap;
        use std::{ffi::CString, os::raw::c_char};",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(1)));
    assert!(lines.ignore.contains(&Lines::Line(2)));
}

#[test]
fn include_inline_fns() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[inline]
            fn inline_func() {
                // I shouldn't be covered
                println!(\"I should\");
                /*
                 None of us should
                 */
                println!(\"But I will\");
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.cover.contains(&3));
    assert!(lines.cover.contains(&4));
    assert!(!lines.cover.contains(&5));
    assert!(!lines.cover.contains(&6));
    assert!(!lines.cover.contains(&7));
    assert!(lines.cover.contains(&8));
}

#[test]
fn cover_callable_noargs() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn foo() {
                std::ptr::null::<i32>();
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(2)));
}

#[test]
fn filter_closure_contents() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn inline_func() {
                (0..0).iter().foreach(|x| {
                    unreachable!();
                    });
            }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(3)));
}

#[test]
fn tarpaulin_skip_attr() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[cfg(not(tarpaulin_include))]
            fn skipped() {
                println!(\"Hello world\");
            }

        #[cfg_attr(tarpaulin, not_a_thing)]
        fn covered() {
            println!(\"hell world\");
        }

        #[cfg(not(tarpaulin))]
        fn uncovered() {
            println!(\"goodbye world\");
        }

        #[tarpaulin::skip]
        fn uncovered2() {
            println!(\"oof\");
        }
        ",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(!lines.ignore.contains(&Lines::Line(7)));
    assert!(!lines.ignore.contains(&Lines::Line(8)));
    assert!(lines.ignore.contains(&Lines::Line(12)));
    assert!(lines.ignore.contains(&Lines::Line(13)));
    assert!(lines.ignore.contains(&Lines::Line(17)));
    assert!(lines.ignore.contains(&Lines::Line(18)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[cfg(not(tarpaulin_include))]
        mod ignore_all {
            fn skipped() {
                println!(\"Hello world\");
            }

            #[cfg_attr(tarpaulin, not_a_thing)]
            fn covered() {
                println!(\"hell world\");
            }
        }
        ",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(8)));
    assert!(lines.ignore.contains(&Lines::Line(9)));
}

#[test]
fn tarpaulin_skip_trait_attrs() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "#[cfg(not(tarpaulin_include))]
            trait Foo {
                fn bar() {
                    println!(\"Hello world\");
                }


                fn not_covered() {
                    println!(\"hell world\");
                }
            }
        ",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(8)));
    assert!(lines.ignore.contains(&Lines::Line(9)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "trait Foo {
                fn bar() {
                    println!(\"Hello world\");
                }

                #[tarpaulin::skip]
                fn not_covered() {
                    println!(\"hell world\");
                }
            }
        ",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(2)));
    assert!(!lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(7)));
    assert!(lines.ignore.contains(&Lines::Line(8)));
}

#[test]
fn tarpaulin_skip_impl_attrs() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "struct Foo;
            #[tarpaulin::skip]
            impl Foo {
                fn bar() {
                    println!(\"Hello world\");
                }


                fn not_covered() {
                    println!(\"hell world\");
                }
            }
        ",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(5)));
    assert!(lines.ignore.contains(&Lines::Line(9)));
    assert!(lines.ignore.contains(&Lines::Line(10)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "struct Foo;
            impl Foo {
                fn bar() {
                    println!(\"Hello world\");
                }


                #[cfg(not(tarpaulin_include))]
                fn not_covered() {
                    println!(\"hell world\");
                }
            }
        ",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(3)));
    assert!(!lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(9)));
    assert!(lines.ignore.contains(&Lines::Line(10)));
}

#[test]
fn filter_block_contents() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn unreachable_match(x: u32) -> u32 {
            match x {
                1 => 5,
                2 => 7,
                #[test]
                _ => {
                    unreachable!();
                },
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(5)));
    assert!(lines.ignore.contains(&Lines::Line(7)));
}

#[test]
fn filter_consts() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn boo() {
        const x: u32 = 3;
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
}

#[test]
fn optional_panic_ignore() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn unreachable_match(x: u32) -> u32 {
            match x {
                1 => 5,
                2 => 7,
                _ => panic!(),
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(!lines.ignore.contains(&Lines::Line(5)));

    let mut config = Config::default();
    config.ignore_panics = true;
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn unreachable_match(x: u32) -> u32 {
            match x {
                1 => 5,
                2 => 7,
                _ => panic!(),
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };

    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(5)));
}

#[test]
fn filter_nested_blocks() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn block() {
            {
                loop {
                    for i in 1..2 {
                        if false {
                            while let Some(x) = Some(6) {
                                while false {
                                    if let Ok(y) = Ok(4) {
                                        unreachable!();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(9)));
}

#[test]
fn filter_multi_line_decls() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn print_it(x:u32,
            y:u32,
            z:u32) {
            println!(\"{}:{}:{}\",x,y,z);
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
    assert!(lines.ignore.contains(&Lines::Line(3)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "struct Boo;
        impl Boo {
            fn print_it(x:u32,
                y:u32,
                z:u32) {
                println!(\"{}:{}:{}\",x,y,z);
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(5)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "trait Boo {
            fn print_it(x:u32,
                y:u32,
                z:u32) {
                println!(\"{}:{}:{}\",x,y,z);
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));
}

#[test]
fn unreachable_propagate() {
    let config = Config::default();
    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "enum Void {}
        fn empty_match(x: Void) -> u32 {
            match x {
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(2)));
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(5)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn foo() {
            if random() {
                loop {
                    match random() {
                        true => match void() {},
                        false => unreachable!()
                    }
                }
            } else {
                call();
            }
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(5)));
    assert!(lines.ignore.contains(&Lines::Line(6)));
    assert!(lines.ignore.contains(&Lines::Line(7)));
    assert!(lines.ignore.contains(&Lines::Line(8)));

    let mut lines = LineAnalysis::new();
    let ctx = Context {
        config: &config,
        file_contents: "fn test_unreachable() {
            let x: u32 = foo();
            if x > 5 {
                bar();
            }
            unreachable!();
        }",
        file: Path::new(""),
        ignore_mods: RefCell::new(HashSet::new()),
    };
    let parser = parse_file(ctx.file_contents).unwrap();
    process_items(&parser.items, &ctx, &mut lines);
    assert!(lines.ignore.contains(&Lines::Line(1)));
    assert!(lines.ignore.contains(&Lines::Line(2)));
    assert!(lines.ignore.contains(&Lines::Line(3)));
    assert!(lines.ignore.contains(&Lines::Line(4)));
    assert!(lines.ignore.contains(&Lines::Line(5)));
    assert!(lines.ignore.contains(&Lines::Line(6)));
    assert!(lines.ignore.contains(&Lines::Line(7)));
}
