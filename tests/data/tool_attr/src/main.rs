#![feature(register_tool)]
#![register_tool(tarpaulin)]

#[tarpaulin::skip]
fn main() {
    println!("Hello, world!");
}
