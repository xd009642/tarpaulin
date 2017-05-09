


pub fn difference(t:f64, t_1:f64, dt:f64) -> f64 {
    if dt == 0.0f64 {
        panic!("Invalid input")
    } else {
        (t - t_1)/dt
    }
}
