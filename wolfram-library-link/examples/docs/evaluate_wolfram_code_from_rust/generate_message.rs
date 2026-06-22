use wolfram_library_link::{
    self as wll, export,
    expr::{expr, Expr},
};

#[export(wstp)]
fn generate_message(_: Vec<Expr>) {
    // Construct the expression `Message[MySymbol::msg, "..."]`, where
    // `MySymbol::msg` is `MessageName[MySymbol, "msg"]`.
    let message = expr!(System::Message[
        System::MessageName[Global::MySymbol, "msg"],
        "a Rust LibraryLink function"
    ]);

    // Evaluate the message expression.
    let _: Expr = wll::evaluate(&message);
}
