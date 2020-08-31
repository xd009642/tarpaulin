#![feature(register_tool)]
#![feature(custom_inner_attributes)]
#![register_tool(tarpaulin)]

#![tarpaulin::skip]


pub fn will_you_ignore() {
    println!("I hope so");
}
