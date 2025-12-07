

mod foo;

pub fn run_it_all() {
    foo::overhere::should_be_in_coverage(); 
    foo::boo::me_as_well();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hundred_percent_coverage() {
        run_it_all();
    }
}
