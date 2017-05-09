

pub mod depend;

use depend::difference;

fn get_diff() {
   println!("{}", difference(1.0f64, 0.20f64, 0.2f64)); 
}



#[cfg(test)]
mod tests {
    use get_diff;
    #[test]
    fn simple() {
        get_diff();
    }
}
