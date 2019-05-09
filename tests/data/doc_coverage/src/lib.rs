


///This is a doc comment
/// ```
/// use doc_coverage::uncovered_by_tests;
/// assert_eq!(4, uncovered_by_tests(4));
/// ```
pub fn uncovered_by_tests(x: i32) -> i32 {
    let y = x.pow(2);
    y / x
}
