///This is a doc comment
/// ```
/// use all_test_types::only_doc_test::*;
/// assert!(only_ran_in_doctest(vec![1,2,3,4,5,6]).is_empty());
/// ```
pub fn only_ran_in_doctest(mut v: Vec<i32>) -> Vec<i32> {
    v.clear();
    v
}
