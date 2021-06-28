
///
///```
///use doctest_env::print_env;
///use std::env;
///
///let print_me = env::var("CARGO_MANIFEST_DIR").unwrap();
///print_env(&print_me);
///```
pub fn print_env(val: &str) {
    println!("{}", val);
}
    
