#![allow(dead_code)]

fn is_even(i: u32) -> bool {
    i%2==0
}


#[test]
fn test_assigns() {
    let _x = "hello there".to_string();
    let _x = String::from("Why hello there");
    let _x = is_even(2);
    let _x = is_even(0) || is_even(2);
    let _x = 2;
    let _x = 2 as u32;
}

#[test]
fn test_assign_ops() {
    let mut x = 0;
    x += 5;
    x *= 2;
    x -= 2;
    x /= 2;
    x %= 2;
    x |= 0xFF;
    x &= 0xCC;
    x <<= 1;
    x >>= 1;

    let _y = x > 2;
    let _y = x < 2;
    let _y = x == 2;
    let _y = x != 2;
    let _y = x <= 2;
    let _y = x >= 2;
    let _y = x | 0x1;
    let _y = x & 0x2;
    let _y = x ^ 2;
    let _y = x + 2;
    let _y = x - 2;
    let _y = x * 2;
    let _y = x / 2;
    let _y = x % 2;
    let _y = (x > 2) && (x < 10);
    let _y = (x > 2) || (x < 10);
    let _y = x << 1;
    let _y = x >> 1;
}
