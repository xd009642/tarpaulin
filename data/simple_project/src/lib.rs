pub mod unused;


pub fn branch_test_one(x: i32) -> i32 {
    if x > 5 {
        10
    } else {
        5
    }
}






#[cfg(test)]
mod tests {
    use branch_test_one;
    #[test]
    fn bad_test() {
        branch_test_one(2);
    }
}
