use std::fmt::Debug;
use std::cmp::Ordering;

pub trait Trait: Debug {
    fn print(&self) {
        println!("{:?}", self);
    }
}

pub struct X<T>(T);

impl<T> X<T>
    where T: Debug{
    fn cmp(&self, other: &T) -> Ordering where T: Ord
    {
        self.0.cmp(other)
    }
}

#[derive(Debug)]
pub struct Y;

impl Y {
    fn f<T>(&self, x: &T)
        where T: Debug,
              Self: Debug,
    {
        println!("{:?} {:?}", self, x);
    }
}

#[inline]
fn hahahaha() {
    println!("BOO");
}

fn gen_print<T>(t: T) 
    where T:Debug {
    println!("I have a {:?}", t);
}

pub fn i_say_i_say() {
    gen_print("Hello there");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        gen_print(7);
    }
}
