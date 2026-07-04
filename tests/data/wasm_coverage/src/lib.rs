pub fn covered(value: u32) -> u32 {
    value + 1
}

pub fn uncovered(value: u32) -> u32 {
    value * 2
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn covered_function_runs() {
        assert_eq!(super::covered(41), 42);
    }
}
