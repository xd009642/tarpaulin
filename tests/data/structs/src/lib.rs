#![allow(dead_code)]

#[derive(Clone, Default, Debug)]
struct Foo {
    x: i32,
    y: Vec<String>,
    z: Option<f64>,
}

impl Foo {
    fn new() -> Self {
        Foo {
            x:5,
            y: vec!["Hello".to_string()],
            z: None
        }
    }
}

#[test]
fn struct_exprs() {
    let _ = Foo::new();
    let _x = Foo {
        x: 6,
        y: vec!["Hello".to_string(),
        "world".to_string(),
        ],
        z: Some(
            0.0)
    };

    let _x = Foo {
        x: 5,
        ..Default::default()
    };
}
