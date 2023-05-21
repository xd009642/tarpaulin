pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_doesnt_work() {
        let result = add(1, 2);
        assert_eq!(result, 4);
    }
}
