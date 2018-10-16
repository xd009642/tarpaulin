#![allow(dead_code)]



#[test]
fn if_test() {
    let x = 5;
    if x%2 == 1 {
        println!("foo");
    }
    if x%2 == x%4 {
    }

    if x > 0 &&
        x - 1 > 0 {
        println!("bar");
    }
}

fn is_even(x: u32) -> bool {
    if x%2 == 0 {
        true
    } else {
        false
    }
}

fn fibonacci(i: i32) -> i32 {
    if i < 0 {
        println!("Invalid");
        i
    } else if i <= 1 {
        i
    } else {
        fibonacci(i - 1) + fibonacci(i - 2)
    }
}

#[test]
fn if_else_test() {
    is_even(2);
    is_even(1);
}


#[test]
fn if_else_if_test() {
    fibonacci(-1);
    fibonacci(2);
    fibonacci(1);
}
