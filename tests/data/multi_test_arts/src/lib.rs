

pub fn test_me(is_on: bool) -> Option<()> {
    if is_on {
        Some(())
    } else {
        None
    }
}


pub fn hello_name(name: String) {
    println!("Hello {}", name);
}
