#![allow(dead_code)]


fn print_length(x: &[i32]) {
    println!("{}", x.len());
}

#[test]
fn simple_creation() {
    // Create arrays and use slices with some line breaks dotted around
    // to make sure no lines are marked as false positives
    let x: [i32; 4] = [1,2,3,4];
    
    print_length(
        &x[0..2]
        );
    
    let _y: [i32; 300] = 
        [0; 300];

    let _y: 
        [i32; 300] = 
        [0; 300];

    let _y =
        x[2];
}
