extern crate multi_test_arts;

use multi_test_arts::*;


#[test]
fn test_on() {
    assert!(test_me(true).is_some());
    assert!(test_me(false).is_none());
}
