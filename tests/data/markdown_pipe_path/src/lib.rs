#[path = "value|helper.rs"]
mod value;

pub fn answer() -> u32 {
    value::answer()
}

#[cfg(test)]
mod tests {
    #[test]
    fn returns_answer() {
        assert_eq!(super::answer(), 42);
    }
}
