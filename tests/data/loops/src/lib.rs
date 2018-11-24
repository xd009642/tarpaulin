#![allow(dead_code)]



#[test]
fn it_works() {
    let mut x = 0i32;
    loop {
        x += 1;
        if x > 10 {
            break;
        }
    }
    
    while x > 0 {
        x -= 1;
    }

    for _y in 0..10 {
        x += 1;
    }
}
