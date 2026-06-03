//! Native-mode runtime: the C-ABI dispatcher the `#[export]` macro calls into
//! for `MArgument`-based functions, plus the `#[init]` helper.
//!
//! Types and helpers (`NativeFunction`, `initialize`, `call_and_catch_as_expr`)
//! are imported from `wolfram-library-link`; the dispatcher logic itself is
//! owned here so the macro emission paths under `wolfram_export::macro_utils::*`
//! resolve without going back through `wolfram-library-link`.

use std::os::raw::c_int;
use std::panic::AssertUnwindSafe;

use wolfram_library_link::macro_utils::call_and_catch_as_expr;
use wolfram_library_link::sys::{self, MArgument};
use wolfram_library_link::NativeFunction;

/// Returned when [`wolfram_library_link::initialize`] fails on entry.
/// Returned when the wrapped Rust code panicked.

/// Bridge a native `#[export]`-marked function across the LibraryLink C ABI.
///
/// 1. Calls `wolfram_library_link::initialize(lib_data)`.
/// 2. Slices `argc` raw `MArgument`s, hands them to the user's
///    `NativeFunction` impl (which performs `FromArg`/`IntoArg` conversions),
///    catching any panic.
pub unsafe fn call_native_wolfram_library_function<'a, F: NativeFunction<'a>>(
    lib_data: sys::WolframLibraryData,
    args: *mut MArgument,
    argc: sys::mint,
    res: MArgument,
    func: F,
) -> c_int {
    if wolfram_library_link::initialize(lib_data).is_err() {
        return wolfram_library_link::FAILED_TO_INIT;
    }

    let argc = match usize::try_from(argc) {
        Ok(argc) => argc,
        Err(_) => {
            return wolfram_library_link::LibraryError::InvalidArgCount.return_code()
        },
    };

    // FIXME: This isn't safe! 'a could be 'static, and then the user could store the
    //        `&mut Link` reference beyond the lifetime of this function.
    //        E.g. `fn foo(link: &'static mut str) { ... }`
    let args: &[MArgument] = std::slice::from_raw_parts(args, argc);

    if call_and_catch_as_expr(AssertUnwindSafe(move || func.call(args, res))).is_err() {
        return wolfram_library_link::FAILED_WITH_PANIC;
    }

    sys::LIBRARY_NO_ERROR as c_int
}

/// Bridge an `#[init]`-marked function: runs `initialize` then the user's
/// init body inside a panic guard.
pub unsafe fn init_with_user_function(
    lib: sys::WolframLibraryData,
    user_init_func: fn(),
) -> c_int {
    if wolfram_library_link::initialize(lib).is_err() {
        return wolfram_library_link::FAILED_TO_INIT;
    }

    if call_and_catch_as_expr(user_init_func).is_err() {
        wolfram_library_link::FAILED_WITH_PANIC
    } else {
        sys::LIBRARY_NO_ERROR as c_int
    }
}
