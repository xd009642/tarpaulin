/// ```
/// use doctest_should_panic::foo;
/// foo();
/// ```
pub fn foo() {
}


/// ```should_panic
/// use doctest_should_panic::bar;
/// bar();
/// ```
pub fn bar() {
    panic!()
}
