#![cfg(not(tarpaulin_include))]

pub mod something;

pub use workspace_1::*;


#[cfg(test)]
mod tests {
    #[test]
    fn it_also_works() {
        assert_eq!(2 + 2, 4);
    }
}
