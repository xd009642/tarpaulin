#![feature(test)]

extern crate test;

#[cfg(test)]
mod tests {
    use test::Bencher;

    #[bench]
    fn it_works(b: &mut Bencher) {
        b.iter(|| println!("benchmark"));
    }
}
