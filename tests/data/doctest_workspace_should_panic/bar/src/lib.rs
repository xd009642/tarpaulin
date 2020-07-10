
/// ```
/// use bar::foo2;
/// foo2();
/// ```
pub fn foo2() {
}


/// ```should_panic
/// use bar::bar2;
/// bar2();
/// ```
pub fn bar2() {
    panic!()
}
