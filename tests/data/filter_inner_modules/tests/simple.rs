#![cfg(not(tarpaulin_include))]

use filter_inner_modules::will_you_ignore;


#[test]
fn simple_test() {
    will_you_ignore();
}
