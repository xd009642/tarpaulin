#![allow(dead_code)]

#[test]
fn test_loop_breaks_no_label_no_value() {
    loop {
        break;
    }

    loop { break }

    loop {
        loop {
            break;
        }
        break;
    }
}


#[test]
fn test_loop_breaks_no_label_but_has_value() {
    let x = loop {
        break 123;
    };

    let x = loop {
        let x = loop {
            break x;
        };
        break x;
    };

    let _x = loop {
        break loop {
            break loop {
                loop {
                    break x; // does nothing really
                };
                break x;
            }
        }
    };
}

#[test]
fn test_for_breaks() {
    for _i in 0..10 {
        break;
    }

    for i in 0..10 {
        if i == 4 {
            break;
        }
    }
}


#[test]
fn test_for_breaks_with_labels() {
    'for1: for _i in 0..10 {
        break 'for1;
    }

    'for2: for i in 0..10 {
        if i == 4 {
            break 'for2;
        }
    }
}


#[test]
fn test_while_breaks() {
    let mut x = 0;

    while x < 10 {
        break;
    }

    while x < 10 {
        x += 1;

        if x == 4 {
            break;
        }
    }
}

#[test]
fn test_while_breaks_with_labels() {
    let mut x = 0;

    'while1: while x < 10 {
        break 'while1;
    }

    'while2: while x < 10 {
        x += 1;

        if x == 6 {
            break 'while2;
        }
    }
}


#[test]
fn test_breaks_label_no_value() {
    'loop1: loop {
        break 'loop1;
    }

    'loop2: loop {
        'loop3: loop {
            break 'loop3;
        }
        break;
    }

    'loop4: loop {
        'loop5: loop {
            break 'loop4;
        }
    }
}


#[test]
fn test_breaks_label_and_value() {
    let x = 'loop1: loop {
        break 'loop1 123;
    };

    let y = 'loop2: loop {
        break ('loop3: loop {
            break 'loop3 321;
        })
    };

    let z = 'loop4: loop {
        'loop5: loop {
            break 'loop4 x;
        }
    };

    assert_eq!(x, z);
}
