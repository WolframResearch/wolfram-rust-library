//! WSTP-mode runtime: the C-ABI dispatcher the `#[export(wstp)]` macro calls
//! into for `LinkObject`-based functions, plus the loader bridge that powers
//! `generate_loader!`.
//!
//! Types and helpers (`WstpFunction`, `initialize`, `call_and_catch_as_expr`)
//! are imported from `wolfram-library-link`; the dispatcher logic itself lives
//! here so the macro paths under `wolfram_export::macro_utils::*` resolve
//! without bouncing back through `wolfram-library-link::macro_utils`.

use std::os::raw::c_int;
use std::panic::AssertUnwindSafe;

use wolfram_library_link::macro_utils::call_and_catch_as_expr;
use wolfram_library_link::sys::{self, LIBRARY_NO_ERROR};
use wolfram_library_link::WstpFunction;
use wstp::Link;

const FAILED_TO_INIT: c_int = 1001;
const FAILED_WITH_PANIC: c_int = 1002;

/// Shared inner helper: initialize, run `function(link)` under a panic guard,
/// and on panic write the resulting `Failure[..]` expression to the link.
unsafe fn call_wstp_link_wolfram_library_function<
    F: FnOnce(&mut Link) + std::panic::UnwindSafe,
>(
    libdata: sys::WolframLibraryData,
    mut unsafe_link: wstp::sys::WSLINK,
    function: F,
) -> c_int {
    if wolfram_library_link::initialize(libdata).is_err() {
        return FAILED_TO_INIT;
    }

    let link = Link::unchecked_ref_cast_mut(&mut unsafe_link);

    let result = call_and_catch_as_expr(AssertUnwindSafe(|| {
        let _: () = function(link);
    }));

    match result {
        Ok(()) => LIBRARY_NO_ERROR as c_int,
        // Try to fail gracefully by writing the panic-as-Failure[..] to the
        // link; if that itself fails we surrender to FAILED_WITH_PANIC.
        Err(failure_expr) => match write_failure_to_link(link, failure_expr) {
            Ok(()) => LIBRARY_NO_ERROR as c_int,
            Err(_wstp_err) => FAILED_WITH_PANIC,
        },
    }
}

fn write_failure_to_link(
    link: &mut Link,
    failure: wolfram_library_link::expr::Expr,
) -> Result<(), wstp::Error> {
    // The panic that brought us here may have been triggered by code like
    // `link.do_something(...).unwrap()`, which would have left the link in
    // an error state. Clear it before we try to put our own expression.
    link.clear_error();

    if link.is_ready() {
        link.raw_get_next()?;
        let result: Result<(), _> = link.new_packet();
        if result.is_err() {
            link.clear_error();
        }
    }

    link.put_expr(&failure)
}

/// Bridge a `#[export(wstp)]`-marked function across the LibraryLink C ABI.
pub unsafe fn call_wstp_wolfram_library_function<
    F: WstpFunction + std::panic::UnwindSafe,
>(
    libdata: sys::WolframLibraryData,
    unsafe_link: wstp::sys::WSLINK,
    func: F,
) -> c_int {
    call_wstp_link_wolfram_library_function(
        libdata,
        unsafe_link,
        move |link: &mut Link| {
            let _: () = func.call(link);
        },
    )
}

/// Body of the `load_library_functions[<path>]` WSTP entry point emitted by
/// `generate_loader!`: read a path argument, build the
/// `<| "name" -> LibraryFunctionLoad[...] |>` Association from the inventory,
/// and write it to the link.
#[cfg(feature = "automate-function-loading-boilerplate")]
pub unsafe fn load_library_functions_impl(
    lib_data: sys::WolframLibraryData,
    raw_link: wstp::sys::WSLINK,
) -> c_int {
    use wolfram_library_link::expr::Expr;
    use wolfram_library_link::LibraryError;

    // A bad call to the generated loader surfaces a structured
    // `Failure["LoaderError", <|…|>]` (with the data behind it) instead of an
    // opaque RustPanic.
    let loader_failure = |link: &mut Link, message: &str, expected: &str, got: Expr| {
        let f = LibraryError::Loader {
            message: message.to_string(),
            expected: expected.to_string(),
            got,
        };
        let _ = write_failure_to_link(link, f.to_expr());
    };

    call_wstp_link_wolfram_library_function(lib_data, raw_link, |link: &mut Link| {
        let arg_count = match link.test_head("List") {
            Ok(n) => n,
            Err(e) => {
                loader_failure(
                    link,
                    "loader call must be List[path]",
                    "List",
                    Expr::string(e.to_string()),
                );
                return;
            },
        };

        if arg_count != 1 {
            loader_failure(
                link,
                "loader takes exactly one argument: the dynamic library path",
                "1 argument",
                Expr::from(arg_count as i64),
            );
            return;
        }

        // Resolve to an owned value first so the `link` borrow from
        // `get_string_ref` ends before we re-borrow `link` to write a failure.
        let path = match link
            .get_string_ref()
            .map(|s| std::path::PathBuf::from(s.as_str()))
            .map_err(|e| e.to_string())
        {
            Ok(p) => p,
            Err(msg) => {
                loader_failure(
                    link,
                    "loader argument must be a String path",
                    "String",
                    Expr::string(msg),
                );
                return;
            },
        };

        let expr =
            ::wolfram_export_core::exported_library_functions_association(Some(path));

        link.put_expr(&expr)
            .expect("failed to write loader Association");
    })
}
