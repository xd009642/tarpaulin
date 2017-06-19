
fn simple_branch(x: u32) {
    if x > 10 {
        println!("Big");
    } else {
        println!("Small");
    }
}


#[cfg(test)]
mod tests {
    use simple_branch;

    #[test]
    fn branch1() {
        simple_branch(0);
    }
    
    #[test]
    fn branch2() {
        simple_branch(11);
    }
}
