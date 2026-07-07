//! `#[export(margs)]` — same C ABI as native mode, but the wrapped function
//! receives the raw `&[MArgument]`/`MArgument` directly instead of having
//! `FromArg`/`IntoArg` applied automatically. Useful when you need manual
//! control over marshaling (e.g. variable arity, or a return type not covered
//! by `IntoArg`).

use wolfram_export::export;
use wolfram_library_link::{sys::MArgument, FromArg, IntoArg, NumericArray};

#[export(margs, args = (::Real, ::Real), ret = ::Real)]
fn margs_add(args: &[MArgument], ret: MArgument) {
    let a = unsafe { f64::from_arg(&args[0]) };
    let b = unsafe { f64::from_arg(&args[1]) };
    unsafe {
        *ret.real = crate::core::add(a, b);
    }
}

#[export(margs,
    args = (
        ::List[::LibraryDataType["NumericArray", "Real64"], "Constant"],
        ::List[::LibraryDataType["NumericArray", "Real64"], "Constant"]
    ),
    ret = ::Real
)]
fn margs_dot(args: &[MArgument], ret: MArgument) {
    let a = unsafe { <&NumericArray<f64>>::from_arg(&args[0]) };
    let b = unsafe { <&NumericArray<f64>>::from_arg(&args[1]) };
    unsafe {
        *ret.real = crate::core::dot(a.as_slice(), b.as_slice());
    }
}

#[export(margs,
    args = (::List[::LibraryDataType["NumericArray", "Real64"], "Constant"], ::Real),
    ret = ::LibraryDataType["NumericArray", "Real64"]
)]
fn margs_scale_array(args: &[MArgument], ret: MArgument) {
    let arr = unsafe { <&NumericArray<f64>>::from_arg(&args[0]) };
    let factor = unsafe { f64::from_arg(&args[1]) };
    let result = crate::core::scale_array(arr.as_slice(), factor);
    unsafe {
        NumericArray::from_slice(&result).into_arg(ret);
    }
}

#[export(margs, args = (::Real), ret = ::Real)]
fn margs_force_panic(args: &[MArgument], _ret: MArgument) {
    let n = unsafe { f64::from_arg(&args[0]) };
    crate::core::force_panic(n);
}
