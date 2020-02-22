

pub fn foo(x: usize) -> usize {
    x % 5
}


pub fn bar(x: usize) -> usize {
    x % 3
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(feature = "feature1")]
    fn foo_run() {
        foo(3);
    }

    #[test]
    #[cfg(feature = "feature2")]
    fn bar_run() {
        bar(2);
    }
}
