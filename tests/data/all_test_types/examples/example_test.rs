use all_test_types::only_example::*;

#[test]
fn example_test() {
    main()
}

fn main() {
    let _ = only_ran_in_examples(vec![1, 2, 43, 4, 5]);
}
