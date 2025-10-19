use std::env;

pub fn add(left: u64, right: u64) -> u64 {
    assert!(env::var("REQUIRED_VAR").is_ok());
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
