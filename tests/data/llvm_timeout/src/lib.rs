pub fn answer() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    use super::answer;
    use std::{thread, time::Duration};

    #[test]
    fn slow_but_successful() {
        thread::sleep(Duration::from_secs(4));
        assert_eq!(answer(), 42);
    }
}
