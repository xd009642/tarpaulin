#[cfg(test)]
mod tests {
    use std::env;

    #[test]
    fn it_works() {
        let args = env::args().collect::<Vec<String>>();
        assert_eq!(args.len(), 1);
    }
}
