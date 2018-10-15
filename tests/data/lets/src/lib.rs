#![allow(dead_code)]

#[cfg(test)]
#[derive(Default)]
struct Foo {
    x: f64,
    y: Option<i32>
}


#[test]
fn let_statements() {
    let _x = 5;
    let _x =
        5;
    let _x
        =
        5;
    let _x:
        i32 
        =
        5;


    let _y: Foo = Foo::default();
    let _y: 
        Foo 
        = Foo::default();
}
