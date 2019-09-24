#![feature(async_await, futures_api)]

#[test]
pub fn a() {
    futures::executor::ThreadPool::new();
}


#[test]
pub fn b() {
    futures::executor::ThreadPool::new();
}
