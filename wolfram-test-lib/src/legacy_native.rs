use wolfram_library_link::{self as wll, NumericArray, UninitNumericArray};

#[wll::export]
fn square(x: i64) -> i64 {
    x * x
}

#[wll::export]
fn add(a: f64, b: f64) -> f64 {
    a + b
}

#[wll::export]
fn reverse_string(s: String) -> String {
    s.chars().rev().collect()
}

#[wll::export]
fn total_i64(list: &NumericArray<i64>) -> i64 {
    list.as_slice().iter().sum()
}

#[wll::export]
fn dot_f64(a: &NumericArray<f64>, b: &NumericArray<f64>) -> f64 {
    a.as_slice()
        .iter()
        .zip(b.as_slice())
        .map(|(x, y)| x * y)
        .sum()
}

#[wll::export]
fn scale_f64(arr: &NumericArray<f64>, factor: f64) -> NumericArray<f64> {
    let result: Vec<f64> = arr.as_slice().iter().map(|x| x * factor).collect();
    NumericArray::from_slice(&result)
}

#[wll::export]
fn positive_i64(list: &NumericArray<i64>) -> NumericArray<u8> {
    let mut out = UninitNumericArray::<u8>::from_dimensions(list.dimensions());
    for (elem, slot) in list.as_slice().iter().zip(out.as_slice_mut()) {
        slot.write(u8::from(elem.is_positive()));
    }
    unsafe { out.assume_init() }
}
