#![allow(dead_code)]

#[test]
fn test_loop_continue_no_label_no_value() {
    let mut x = 0;
    loop {
        if x > 1 {
            break;
        } else {
            x += 1;
            continue;
        }
    }
}

#[test]
fn test_for_continues() {
    for _i in 0..10 {
        continue;
    }

    let mut v = vec![];
    for i in 0..10 {
        if i % 2 == 0 {
            continue;
        } else {
            v.push(i);
        }
    }

    assert_eq!(v, [1, 3, 5, 7, 9]);
}

#[test]
fn test_for_continue_with_labels() {
    'for1: for _i in 0..10 {
        continue 'for1;
    }

    let mut v = vec![];
    'for2: for i in 0..10 {
        if i % 2 == 0 {
            continue 'for2;
        } else {
            v.push(i);
        }
    }

    assert_eq!(v, [1, 3, 5, 7, 9]);

    let mut v = vec![];
    'x: for x in 0..4 {
        'y: for y in 0..4 {
            if x % 2 == 0 {
                continue 'x;
            }
            if y % 2 == 0 {
                continue 'y;
            }
            v.push((x, y));
        }
    }
    assert_eq!(v, [(1, 1), (1, 3), (3, 1), (3, 3)]);
}

#[test]
fn test_while_continues() {
    let mut x = 0;

    let mut v = vec![];
    while x < 10 {
        x += 1;
        if x % 2 == 0 {
            continue;
        } else {
            v.push(x);
        }
    }

    assert_eq!(v, [1, 3, 5, 7, 9]);
}

#[test]
fn test_while_continues_with_labels() {
    let mut x = 0;

    let mut v = vec![];
    'while1: while x < 10 {
        x += 1;
        if x % 2 == 0 {
            continue 'while1;
        } else {
            v.push(x);
        }
    }

    assert_eq!(v, [1, 3, 5, 7, 9]);

    let mut x = 0;
    let mut v = vec![];
    'x: while x < 4 {
        x += 1;
        let mut y = 0;
        'y: while y < 4 {
            y += 1;
            if x % 2 == 0 {
                continue 'x;
            }
            if y % 2 == 0 {
                continue 'y;
            }
            v.push((x, y));
        }
    }
    assert_eq!(v, [(1, 1), (1, 3), (3, 1), (3, 3)]);
}
