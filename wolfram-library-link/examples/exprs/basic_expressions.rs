use wolfram_library_link::{
    self as wll,
    expr::{expr, Expr},
};

/// This function is loaded by evaluating:
///
/// ```wolfram
/// LibraryFunctionLoad[
///     "/path/to/libbasic_expressions.dylib",
///     "echo_arguments",
///     LinkObject,
///     LinkObject
/// ]
/// ```
#[wll::export(wstp)]
pub fn echo_arguments(args: Vec<Expr>) -> Expr {
    let arg_count = args.len();

    for arg in args {
        // Echo[<arg>]
        wll::evaluate(&expr!(System::Echo[arg]));
    }

    Expr::string(format!("finished echoing {} argument(s)", arg_count))
}
