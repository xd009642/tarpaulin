

/// Checks if things are negative
///
/// ```
/// use doctest::is_negative;
/// let x = is_negative(-1);
/// ```
pub fn is_negative(i: i32) -> bool {
    if i < 0 {
        true
    } else {
        false
    }
}



#[cfg(test)]
mod tests {
    use is_negative;
    #[test]
    fn branch1() {
        is_negative(1);
    }
}
