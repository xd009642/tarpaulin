
#[cfg(config1)]
fn config1_hello() {
    println!("Hello!");
}

#[cfg(config2)]
fn config2_hello() {
    println!("Hello!");
}


#[test]
#[cfg(config1)]
fn it_works() {
    config1_hello();
}

#[test]
#[cfg(config2)]
fn it_works() {
    config2_hello();
}
