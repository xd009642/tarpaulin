/// ```
/// use foo_bar::foo1;
/// foo1();
/// ```
pub fn foo1() {
}


/// ```should_panic
/// use foo_bar::bar1;
/// bar1();
/// ```
pub fn bar1() {
    panic!()
}
