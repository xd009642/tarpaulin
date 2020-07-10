/// ```
/// use foo::foo1;
/// foo1();
/// ```
pub fn foo1() {
}


/// ```should_panic
/// use foo::bar1;
/// bar1();
/// ```
pub fn bar1() {
    panic!()
}
