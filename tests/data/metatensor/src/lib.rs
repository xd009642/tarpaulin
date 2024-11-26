#[cfg(test)]
mod tests {
    #[test]
    fn test_labels() {
        let labels = metatensor::Labels::new(["a"], &[[0]]);
        assert_eq!(labels.names(), ["a"]);
    }
}
