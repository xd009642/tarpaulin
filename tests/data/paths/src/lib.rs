use std::cmp::{
    min,
    max
};

pub fn junk() -> Vec<u8> {
    Vec::<u8>::with_capacity(max(1, min(10, 0)))
}


#[test]
fn it_works() {
    junk();
}
