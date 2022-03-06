use std::io::Write;

pub fn do_the_thing() {
    let mut f = std::fs::File::create("/tmp/whatever").unwrap();
    f.write("hello\n".as_bytes()).unwrap();
}
