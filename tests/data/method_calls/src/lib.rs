#![allow(dead_code)]

// NOTE: Do not rustfmt this file. The expressions are specifically placed on a
// single line for testing purposes.

use std::fs::File;
use std::io::Read;

#[test]
fn simple_method_call() {
    let _ascii = "a".is_ascii();
    let _ascii = "a"
        .is_ascii();

    let _equal = "Ferrös".eq_ignore_ascii_case("FERRöS");
    let _equal = "Ferrös"
        .eq_ignore_ascii_case("FERRöS");

    let _is_empty = Vec::<i32>::new().is_empty();
    let _is_empty = Vec::<i32>::new()
        .is_empty();

    let _is_4_in_list = vec![1, 2, 3, 4].contains(&4);
    let _is_4_in_list = vec![1, 2, 3, 4]
        .contains(&4); // FIXME: This line is not covered.

    let _is_4_in_list = Vec::new().contains(&4);
    let _is_4_in_list = Vec::new()
        .contains(&4); // FIXME: This line is not covered.
}

#[test]
fn simple_method_call_with_try() -> Result<(), Box<dyn std::error::Error>> {
    let _num: i32 = "123".parse()?;

    let mut test = File::open("Cargo.toml")?;
    let mut string = String::new();
    test.read_to_string(&mut string)?;

    // Separated lines
    let _num: i32 = "123"
        .parse()?;

    let mut test = File::open("Cargo.toml")?;
    let mut string = String::new();
    test
        .read_to_string(&mut string)?; // FIXME: This line is not covered.

    Ok(())
}

#[test]
fn method_call_chain() {
    let num: &str = "123".clone().trim();
    let _truth = num.contains("23").to_string();
    let _is_4_in_list: bool = vec![1, 12, 4, 7, 5, 5, 0, 3].as_slice().iter().filter(|&x| x%2 != 0).map(|&x| x).collect::<Vec<_>>().contains(&&4);

    // Separated lines
    let num: &str = "123"
        .clone()
        .trim();

    let _truth = num
        .contains("23")
        .to_string();

    // Progressively building more complex chains for testing...
    let _is_4_in_list = Vec::new()
        .iter()
        .collect::<Vec<_>>()
        .contains(&&4); // FIXME: This line is not covered.

    let _is_4_in_list = vec![1, 2, 3, 4]
        .iter()
        .map(|&x| x)
        .collect::<Vec<_>>()
        .contains(&&4); // FIXME: This line is not covered.

    let _is_4_in_list = vec![1, 2, 3, 4]
        .iter()
        .filter(|&x| x%2 != 0)
        .map(|&x| x)
        .collect::<Vec<_>>()
        .contains(&&4); // FIXME: This line is not covered.

    let _is_4_in_list = vec![1, 2, 3, 4]
        .as_slice()
        .iter()
        .filter(|&x| x%2 != 0)
        .map(|&x| x)
        .collect::<Vec<_>>()
        .contains(&&4); // FIXME: This line is not covered.

    let _is_4_in_list = vec![1, 2, 3, 4]
        .as_slice()
        .iter()
        .filter(|&x| x%2 != 0)
        .map(|&x| x)
        .collect::<Vec<_>>()
        .contains(&&4)
        .to_string(); // FIXME: This line is not covered.

    let _is_4_in_list = vec![1, 2, 3, 4]
        .as_slice()
        .iter()
        .filter(|&x| x%2 != 0)
        .map(|&x| x)
        .collect::<Vec<_>>()
        .contains(&&4)
        .to_string()
        .clone();

    let _is_4_in_list = vec![1, 2, 3, 4]
        .as_slice()
        .iter()
        .filter(|&x| x%2 != 0)
        .map(|&x| x)
        .collect::<Vec<_>>()
        .contains(&&4)
        .to_string()
        .clone()
        .trim(); // FIXME: This line is not covered.
}

#[test]
fn method_call_chain_with_try() -> Result<(), Box<dyn std::error::Error>> {
    let _num: i32 = "123".clone().parse()?;
    let _num: i32 = "123"
        .clone()
        .parse()?;

    let _num: i32 = "123".parse::<i32>()?.clone();
    let _num: i32 = "123"
        .parse::<i32>()?
        .clone();

    let _num: i32 = "123".clone().parse::<i32>()?.clone();
    let _num: i32 = "123"
        .clone()
        .parse::<i32>()?
        .clone();

    let _num: i32 = "123"
        .clone()
        .parse::<i32>()?
        .reverse_bits()
        .clone()
        .reverse_bits()
        .clone()
        .to_string()
        .parse::<i32>()?; // FIXME: This line is not covered.

    let _num: i32 = "123"
        .clone()
        .parse::<i32>()?
        .reverse_bits()
        .clone()
        .reverse_bits()
        .clone()
        .to_string()
        .parse::<i32>()?
        .clone(); // FIXME: This line is not covered.

    Ok(())
}

// This test ensures that ".collect::<_>()?" should work okay under any
// circumstances that it is used.
#[test]
fn method_call_with_collect_try() -> Result<(), Box<dyn std::error::Error>> {
    let strings = vec!["93", "18"];

    let _collect_no_try = strings.iter().map(|s| s.parse::<i32>()).collect::<Result<Vec<_>, _>>();
    let _collect_no_try = strings
        .iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>();

    let _collect_with_try = strings.iter().map(|s| s.parse::<i32>()).collect::<Result<Vec<_>, _>>()?;
    let _collect_with_try = strings
        .iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()?; // FIXME: This line is not covered.

    let _collect_no_try = vec!["93", "18"].into_iter().map(|s| s.parse::<i32>()).collect::<Result<Vec<_>, _>>();
    let _collect_no_try = vec!["93", "18"]
        .into_iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>();

    let _collect_with_try = vec!["93", "18"].into_iter().map(|s| s.parse::<i32>()).collect::<Result<Vec<_>, _>>()?;
    let _collect_with_try = vec!["93", "18"]
        .into_iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}
