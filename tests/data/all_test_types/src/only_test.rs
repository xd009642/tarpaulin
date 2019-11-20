pub fn only_ran_in_test(mut v: Vec<i32>) -> Vec<i32> {
    v.clear();
    v
}

#[cfg(test)]
pub mod tests {
    use super::*;
    #[test]
    fn test_it() {
        only_ran_in_test(vec![1, 2, 3, 4, 5, 6]);
    }
}
