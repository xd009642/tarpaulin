#[derive(Clone)]
pub struct A {
    x: Box<usize>,
}

#[derive(Clone)]
pub struct B<'a> {
    y: Box<&'a usize>,
}

pub fn return_x1(a: &A) -> usize {
    let _x = A { x: Box::new(0) };
    *a.x
}

pub fn return_x2(a: A) -> usize {
    let _x = B { y: Box::new(&a.x) };

    *a.x
}

pub fn return_y1<'a>(b: &'a B) -> &'a usize {
    let _x = (A { x: Box::new(0) }, B { y: Box::new(&b.y) });

    *b.y
}

pub fn return_y2(b: B) -> usize {
    *(*b.y)
}

#[test]
fn test_boxing() {
    let a = A { x: Box::new(0) };

    let val: usize = 1;
    let b = B { y: Box::new(&val) };

    assert_eq!(return_x1(&a), *a.x);
    assert_eq!(return_x2(a.clone()), *a.x);
    assert_eq!(return_y1(&b), *b.y);
    assert_eq!(return_y2(b.clone()), *(*b.y));
}
