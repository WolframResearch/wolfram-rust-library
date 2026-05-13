use std::os::raw::c_int;

#[cfg(feature = "wstp")]
use wstp::{self, Link};

use crate::{
    catch_panic::call_and_catch_panic,
    expr::Expr,
    sys::{self, MArgument},
    NativeFunction,
};
#[cfg(feature = "wstp")]
use crate::{catch_panic::CaughtPanic, sys::LIBRARY_NO_ERROR, WstpFunction};

/// Error codes returned by macro-generated wrapper code.
///
/// If no error occured, [`sys::LIBRARY_NO_ERROR`] is returned.
///
/// Using separate error codes for macro-generated code makes the source of the error
/// clearer when something goes wrong in wrapper code.
//
// TODO: Make this module public somewhere and document these error code in #[export(..)]
//       and Overview.md.
mod error_code {
    use std::os::raw::c_int;

    // Chosen arbitrarily. Avoids clashing with `LIBRARY_FUNCTION_ERROR` and related
    // error codes.
    const OFFSET: c_int = 1000;

    /// A call to [initialize()][crate::initialize] failed.
    pub const FAILED_TO_INIT: c_int = OFFSET + 1;

    /// The library code panicked.
    //
    // TODO: Wherever this code is set, also set a $LastError-like variable.
    pub const FAILED_WITH_PANIC: c_int = OFFSET + 2;
}

//==================
// Shared panic helper
//==================

/// Run `func`, catch any panic, and convert it to a `Failure[...]` [`Expr`].
///
/// Returns `Ok(T)` on success or `Err(failure_expr)` on panic. Each backend
/// decides what to do with the failure: WXF serializes it, WSTP writes it to
/// the link, native re-panics.
pub fn call_and_catch_as_expr<T, F>(func: F) -> Result<T, Expr>
where
    F: FnOnce() -> T + std::panic::UnwindSafe,
{
    call_and_catch_panic(func).map_err(|caught| caught.to_pretty_expr())
}

//==================
// WSTP helpers
//==================

#[cfg(feature = "wstp")]
unsafe fn call_wstp_link_wolfram_library_function<
    F: FnOnce(&mut Link) + std::panic::UnwindSafe,
>(
    libdata: sys::WolframLibraryData,
    mut unsafe_link: wstp::sys::WSLINK,
    function: F,
) -> c_int {
    // Initialize the library.
    if crate::initialize(libdata).is_err() {
        return error_code::FAILED_TO_INIT;
    }

    let link = Link::unchecked_ref_cast_mut(&mut unsafe_link);

    let result: Result<(), CaughtPanic> =
        call_and_catch_panic(std::panic::AssertUnwindSafe(|| {
            let _: () = function(link);
        }));

    match result {
        Ok(()) => LIBRARY_NO_ERROR as c_int,
        // Try to fail gracefully by writing the panic message as a Failure[..] object to
        // be returned, but if that fails, just return LIBRARY_FUNCTION_ERROR.
        Err(panic) => match write_panic_failure_to_link(link, panic) {
            Ok(()) => LIBRARY_NO_ERROR as c_int,
            Err(_wstp_err) => {
                // println!("PANIC ERROR: {}", _wstp_err);
                error_code::FAILED_WITH_PANIC
            },
        },
    }
}

#[cfg(feature = "wstp")]
fn write_panic_failure_to_link(
    link: &mut Link,
    caught_panic: CaughtPanic,
) -> Result<(), wstp::Error> {
    // Clear the last error on the link, if any.
    //
    // This is necessary because the panic we caught might have been caused by
    // code like:
    //
    //     link.do_something(...).unwrap()
    //
    // where `do_something()` fails, which will have "poisoned" the link, and would cause
    // our attempt to write the panic message to the link to fail if we didn't clear the
    // error.
    //
    // If there is no error condition set on the link, this is a no-op.
    //
    // TODO: If an error *is* set, mention that in the Failure message? That might help
    //       users debug link issues more quickly.
    link.clear_error();

    // Skip whatever data is still stored in the link, if any.
    if link.is_ready() {
        link.raw_get_next()?;

        // Skip to the next packet on the link.
        //
        // If there is (possibly partial) data that is unread, this will
        // skip to the end and return Ok. If there is partially complete data
        // being *written*, this will still skip to the end, but will return
        // an Err(..).
        //
        // Incomplete data being read typically happens if an unwrap()
        // fails when expecting to read an argument of a specific type.
        //
        // Incomplete data being written typically happens if a panic occurs
        // within the logic that puts the function return value. E.g.:
        //
        //     link.put_function("List", 3)?; // Start writing a function of 3 elems
        //     todo!() // <-- leave the List[... incomplete.

        let result: Result<(), _> = link.new_packet();

        if result.is_err() {
            link.clear_error();
        }
    }

    link.put_expr(&caught_panic.to_pretty_expr())
}

//======================================
// #[export] (NativeFunction) and #[export(wstp)] (WstpFunction) helpers
//======================================

pub unsafe fn call_native_wolfram_library_function<'a, F: NativeFunction<'a>>(
    lib_data: sys::WolframLibraryData,
    args: *mut MArgument,
    argc: sys::mint,
    res: MArgument,
    func: F,
) -> c_int {
    use std::panic::AssertUnwindSafe;

    // Initialize the library.
    if crate::initialize(lib_data).is_err() {
        return error_code::FAILED_TO_INIT;
    }

    let argc = match usize::try_from(argc) {
        Ok(argc) => argc,
        Err(_) => return sys::LIBRARY_FUNCTION_ERROR as c_int,
    };

    // FIXME: This isn't safe! 'a could be 'static, and then the user could store the
    //        `&mut Link` reference beyond the lifetime of this function.
    //        E.g. `fn foo(link: &'static mut str) { ... }`
    let args: &[MArgument] = std::slice::from_raw_parts(args, argc);

    if call_and_catch_panic(AssertUnwindSafe(move || func.call(args, res))).is_err() {
        // TODO: Store the panic into a "LAST_ERROR" static, and provide an accessor to
        //       get it from WL? E.g. RustLink`GetLastError[<optional func name>].
        return error_code::FAILED_WITH_PANIC;
    };

    sys::LIBRARY_NO_ERROR as c_int
}

#[cfg(feature = "wstp")]
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

//======================================
// Automatic Loader
//======================================

/// Inventory entry for a `#[export]`-marked function.
///
/// Now a type alias for [`wolfram_export_core::ExportEntry`]. The underlying
/// type is shared across all three export modes (Native, Wstp, Wxf) so the
/// `__wolfram_manifest__` symbol can see every entry regardless of which
/// macro produced it.
pub type LibraryLinkFunction = ::wolfram_export_core::ExportEntry;

// The `inventory::collect!(ExportEntry)` declaration lives in
// `wolfram-export-core`. Don't declare it again here — a duplicate collect!
// would split the inventory.

#[cfg(all(feature = "automate-function-loading-boilerplate", feature = "wstp"))]
pub unsafe fn load_library_functions_impl(
    lib_data: sys::WolframLibraryData,
    raw_link: wstp::sys::WSLINK,
) -> c_int {
    call_wstp_link_wolfram_library_function(lib_data, raw_link, |link: &mut Link| {
        let arg_count: usize =
            link.test_head("List").expect("expected 'List' expression");

        if arg_count != 1 {
            panic!(
                "expected 1 argument: the name of or file path to the dynamic library"
            );
        }

        let path = {
            let path = match link.get_string_ref() {
                Ok(value) => value,
                Err(err) => panic!("expected String argument (error: {})", err),
            };
            std::path::PathBuf::from(path.as_str())
        };

        let expr = exported_library_functions_association(Some(path));

        link.put_expr(&expr)
            .expect("failed to write loader Association");
    })
}

/// Returns an [`Association`][Association] containing the names and `LibraryFunctionLoad`
/// calls for every function in this library marked with [`#[export(..)]`][crate::export].
///
/// The expression returned by this function will automatically load the functions
/// exported by this library. This frees the library author from having to manually write
/// [`LibraryFunctionLoad[..]`][LibraryFunctionLoad] calls for each function.
///
/// See also: [`generate_loader!`][crate::generate_loader]
///
/// ### Possible issues
///
/// <details>
///   <summary>
///     <h6 style="display: inline"><u>Automatic Discovery of Dynamic Library Path Fails</u></h6>
///   </summary>
///
/// This function generates calls to
/// [`LibraryFunctionLoad[lib, ...]`][LibraryFunctionLoad]
/// automatically. The `lib` argument must be a library name or file path that
/// the Wolfram Language can locate using [`FindLibrary`][FindLibrary].
///
/// [`exported_library_functions_association()`] will attempt to determine the
/// `lib` file path automatically at runtime. (This is currently done using
/// [`process_path::get_dylib_path()`](https://docs.rs/process_path/0.1.4/process_path/fn.get_dylib_path.html)
/// ). However, determining this location automatically is not guaranteed to be
/// supported on all operating systems and for all libraries.
///
/// In the event that automatic discovery of the dynamic library file path fails,
/// you can specify the library name / path by specifing it as an argument
/// to [`exported_library_functions_association()`]:
///
/// ```
/// use std::path::PathBuf;
/// # use wolfram_library_link::{exported_library_functions_association, expr::Expr};
///
/// // Specify a library base name. (FindLibrary will search on $LibraryPath and in paclets.)
/// # fn a() -> Expr {
/// exported_library_functions_association(Some(PathBuf::from("my_library")))
/// # }
///
/// // Specify an absolute path
/// # fn b() -> Expr {
/// exported_library_functions_association(Some(PathBuf::from("/Some/Path/To/libmy_library.dylib")))
/// # }
/// ```
///
/// [FindLibrary]: https://reference.wolfram.com/language/ref/FindLibrary.html
///
/// </details>
///
/// # Example
///
/// Suppose that a library exports two functions:
///
/// ```
/// # mod scope {
/// use wolfram_library_link::export;
///
/// #[export]
/// fn square(x: i64) -> i64 {
///     x * x
/// }
///
/// #[export]
/// fn string_join(mut a: String, b: String) -> String {
///     a.push_str(&b);
///     a
/// }
/// # }
/// ```
///
/// If called inside this library, `exported_library_functions_association()` will
/// return the expression:
///
/// ```wolfram
/// <|
///     "square" -> LibraryFunctionLoad[
///         "<library path>",
///         "square",
///         {Integer},
///         Integer
///     ],
///     "string_join" -> LibraryFunctionLoad[
///         "<library path>",
///         "string_join",
///         {String, String},
///         String
///     ]
/// |>
/// ```
///
/// The returned Association automatically contains the boilerplate Wolfram Language code
/// necessary to load the functions exported by this library.
///
/// See also: [`NativeFunction::signature()`]
///
/// # Creating a loader function
///
/// `exported_library_functions_association()` is intended to be used to define a *loader
/// function*. Conventionally, a loader function is just a function that loads the other
/// functions exported by the library.
/// LibraryLink libraries that use the loader function convention will only require that a
/// single `LibraryFunctionLoad` call be written manually. The other calls will be
/// performed automatically.
///
/// To define a loader function, use [`#[export(wstp)]`][crate::export#exportwstp] to
/// export a new function that calls `export_library_functions_association()`.
///
/// ```
/// # mod scope {
/// use wolfram_library_link::{self as wll, export, expr::Expr};
///
/// #[export(wstp, hidden)]
/// fn load_library_functions(args: Vec<Expr>) -> Expr {
///     assert!(args.len() == 0);
///     return wll::exported_library_functions_association(None);
/// }
/// # }
/// ```
///
/// *Note: the `hidden` argument to `export(..)` prevents the loader function itself from
/// appearing in the output of `exported_library_functions_association()`, which would be
/// redundant.*
///
/// Then, in your Wolfram Language code you can write a single `LibraryFunctionLoad` call
/// to manually load the loader function:
///
/// ```wolfram
/// loadLibraryFunctions = LibraryFunctionLoad[
///     "<library path>",
///     "load_library_functions",
///     LinkObject,
///     LinkObject
/// ];
///
/// $functions = loadLibraryFunctions[];
/// ```
///
/// `$functions` will be the Association containing the library functions.
///
/// You can then use `$functions` to access the other exported functions:
///
/// ```wolfram
/// square = $functions["square"]
/// stringJoin = $functions["string_join"]
/// ```
///
/// The loaded functions can be called as normal:
///
/// ```wolfram
/// square[2]    (* Returns 4)
///
/// stringJoin["hello", "world"]    (* Returns "helloworld" *)
/// ```
///
// TODO: Polish this section and make into a doc comment.
// ## Advantages
//
// Using the loader function convention has a number of advantages over writing
// `LibraryFunctionLoad` calls manually:
//
// * Saves time
// * Only one place needs to be updated when the function type signature changes
// * Prevents potential undefined behavior if the type signature used to load the function
//   differs from the definition.
// * Most efficient library type is used automatically (memory management strategy for
//   NumericArray's)
///
/// # Note on semver compatibility
///
/// The only backwards-compatibility guarantee provided by this function is that it
/// returns an Association of the form:
///
/// ```wolfram
/// <| ( name_?StringQ -> func_ )... |>
/// ```
///
/// where `name` is the exported name of the function and `func` is an expression that will
/// call the library function when arguments are applied to it. No specific guarantee is
/// made about what form `func` is.
///
/// `func` is _currently_ a `LibraryFunction[..]` expression for native functions, and a
/// `Function[..]` expression for WSTP functions, but this is not guaranteed to stay
/// unchanged between semver compatible version numbers of this library.
///
/// Callers should treat `func` as an opaque expression that they can apply arguments to.
///
/// [Association]: https://reference.wolfram.com/language/ref/Association.html
/// [LibraryFunctionLoad]: https://reference.wolfram.com/language/ref/LibraryFunctionLoad.html
// The manifest builder lives in `wolfram-export-core`. Re-export under the
// historic name so existing callers (`wolfram_library_link::macro_utils::
// exported_library_functions_association`) keep building.
#[cfg(feature = "automate-function-loading-boilerplate")]
pub use ::wolfram_export_core::exported_library_functions_association;

//======================================
// Initialization
//======================================

pub unsafe fn init_with_user_function(
    lib: sys::WolframLibraryData,
    user_init_func: fn(),
) -> c_int {
    if let Err(()) = crate::initialize(lib) {
        return error_code::FAILED_TO_INIT as c_int;
    }

    if let Err(_) = call_and_catch_panic(user_init_func) {
        error_code::FAILED_WITH_PANIC as c_int
    } else {
        sys::LIBRARY_NO_ERROR as c_int
    }
}
