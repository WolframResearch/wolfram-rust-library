//! `#[export(margs)]` — same C ABI as native mode, but the wrapped function
//! receives the raw `&[MArgument]`/`MArgument` directly instead of having
//! `FromArg`/`IntoArg` applied automatically. Useful when you need manual
//! control over marshaling (e.g. variable arity, or a return type not covered
//! by `IntoArg`) — including types with no `FromArg`/`IntoArg` impl at all,
//! like `SparseArray` (see `margs_sparse_array_merge` below, which reads the
//! raw `MArgument.sparse` pointer and drives the
//! `WolframSparseLibrary_Functions` C API directly).

use wolfram_export::export;
use wolfram_library_link::{
    rtl,
    sys::{mint, MArgument, MSparseArray, MTensor, MType_Real},
    FromArg, IntoArg, NumericArray,
};

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

#[export(margs,
    args = (::LibraryDataType[::SparseArray], ::LibraryDataType[::SparseArray]),
    ret = ::LibraryDataType[::SparseArray]
)]
fn margs_sparse_array_merge(args: &[MArgument], ret: MArgument) {
    // No `FromArg`/`IntoArg` impl exists for `SparseArray`, so this is handled by
    // hand, reading the raw `MSparseArray` pointer out of the `MArgument` union
    // and driving the `WolframSparseLibrary_Functions` C API directly.
    //
    // Merges `a` and `b` (which must have equal rank and dimensions) into a new
    // sparse array: wherever `b` has a nonzero entry it wins, otherwise `a`'s
    // entry (possibly the 0. background) is kept.
    unsafe {
        let a: MSparseArray = *args[0].sparse;
        let b: MSparseArray = *args[1].sparse;

        let rank = rtl::MSparseArray_getRank(a);
        if rtl::MSparseArray_getRank(b) != rank {
            panic!("sparse arrays must have the same rank");
        }

        let dims_a =
            std::slice::from_raw_parts(rtl::MSparseArray_getDimensions(a), rank as usize);
        let dims_b =
            std::slice::from_raw_parts(rtl::MSparseArray_getDimensions(b), rank as usize);
        if dims_a != dims_b {
            panic!("sparse arrays must have the same dimensions");
        }

        let mut tensor_a: MTensor = std::ptr::null_mut();
        let mut tensor_b: MTensor = std::ptr::null_mut();
        if rtl::MSparseArray_toMTensor(a, &mut tensor_a) != 0 {
            panic!("failed to convert first sparse array to a dense tensor");
        }
        if rtl::MSparseArray_toMTensor(b, &mut tensor_b) != 0 {
            panic!("failed to convert second sparse array to a dense tensor");
        }

        let len = rtl::MTensor_getFlattenedLength(tensor_a) as usize;
        let data_a = rtl::MTensor_getRealData(tensor_a);
        let data_b = rtl::MTensor_getRealData(tensor_b);

        let mut merged: MTensor = std::ptr::null_mut();
        if rtl::MTensor_new(MType_Real as mint, rank, dims_a.as_ptr(), &mut merged) != 0 {
            panic!("failed to allocate the merged tensor");
        }
        let merged_data = rtl::MTensor_getRealData(merged);
        for i in 0..len {
            let b_value = *data_b.add(i);
            *merged_data.add(i) = if b_value != 0.0 {
                b_value
            } else {
                *data_a.add(i)
            };
        }

        // `MSparseArray_fromMTensor`'s implicit-value argument must be null or a
        // rank-0 tensor; null means "background value 0.", which is what we want.
        let mut result: MSparseArray = std::ptr::null_mut();
        let err =
            rtl::MSparseArray_fromMTensor(merged, std::ptr::null_mut(), &mut result);

        rtl::MTensor_free(tensor_a);
        rtl::MTensor_free(tensor_b);

        if err != 0 {
            panic!("failed to build the merged sparse array");
        }

        *ret.sparse = result;
    }
}

#[export(margs, args = (::Real), ret = ::Real)]
fn margs_force_panic(args: &[MArgument], _ret: MArgument) {
    let n = unsafe { f64::from_arg(&args[0]) };
    crate::core::force_panic(n);
}
