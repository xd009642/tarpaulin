unsafe extern "C" {
    static FIRST: u8;
    static SECOND: u8;
}

pub fn linker_symbols() -> (usize, usize) {
    (std::ptr::addr_of!(FIRST) as usize, std::ptr::addr_of!(SECOND) as usize)
}

#[cfg(test)]
mod tests {
    /// Both linker arguments are needed to define the symbols used by the library.
    #[test]
    fn both_linker_arguments_are_used() {
        let (first, second) = super::linker_symbols();
        assert_eq!(second - first, 11);
    }
}
