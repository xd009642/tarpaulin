#[cfg(feature = "optional")]
pub mod optional;

#[cfg(feature = "feature_c")]
pub mod transitive;

pub fn covered() -> u8 {
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn covers_public_api() {
        assert_eq!(crate::covered(), 1);
    }
}
