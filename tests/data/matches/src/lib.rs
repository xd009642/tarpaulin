
pub fn check_match(x: usize) -> usize {
    match x {
        0 => 1,
        1...5 => 2,
        6 | 8 => 3,
        x if x % 2 == 0 => x,
        _ => 0,
    }
}

pub fn destructuring_match(x: u32, y: u32) {
    let _y = match (x, y) {
        (1, _) => 1,
        (_, 1) => 1,
        (2, 2) => 2,
        _ => 0,
    };

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        check_match(0);
        check_match(2);
        check_match(999999);
        check_match(8);
        check_match(9998);

        destructuring_match(1, 3);
        destructuring_match(2, 1);
        destructuring_match(2, 2);
        destructuring_match(3, 2);
    }
}
