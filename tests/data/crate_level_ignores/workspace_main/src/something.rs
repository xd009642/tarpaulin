

fn uncovered_function(len: usize, inc: i32) -> Vec<i32> {
    let mut x = vec![];
    for i in 0..len {
        let val = i as i32 * inc;
        x.push(val)
    }
    x
}
