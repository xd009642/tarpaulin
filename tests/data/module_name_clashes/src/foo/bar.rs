

#[cfg(test)]
mod overhere {
    mod boo;

    #[test] 
    fn excluded() {
        println!("I work");
    }
}
