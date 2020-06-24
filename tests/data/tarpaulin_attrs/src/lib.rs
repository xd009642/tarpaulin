


#[test]
#[cfg_attr(tarpaulin, ignore)]
fn it_works() {
    assert_eq!(2 + 2, 4);
}

#[test]
#[cfg(not(tarpaulin))]
fn it_works2() {
    assert_eq!(2 + 2, 4);
}

#[test]
#[cfg(tarpaulin_skip)]
fn it_works2() {
    assert_eq!(2 + 2, 4);
}
