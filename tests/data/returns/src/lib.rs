#![allow(dead_code)]

fn early_return(i: bool) -> i32 {
    if i {
        return 1
    }

    0
}

fn is_even(i: i32) -> bool {
    if i%2 == 0 {
        true
    } else {
        false
    }
}

#[test]
fn return_statements() {
    early_return(true);
    early_return(false);

    is_even(1);
    is_even(2);
}
