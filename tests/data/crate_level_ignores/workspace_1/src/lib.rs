
pub mod foo;

fn print_hello() {
    println!("HellO");
}

fn multiply(x: i32, y: i32) -> i32 {
    x * y
}

#[cfg(test)]
mod tests {
    pub use super::*; 

    #[test]
    fn it_works() {
        print_hello();
        assert_eq!(6, multiply(2, 3));
    }
}
