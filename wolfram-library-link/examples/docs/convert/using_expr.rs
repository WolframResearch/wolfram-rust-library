use wolfram_library_link::{
    export,
    expr::{expr, Expr},
};

struct Point {
    x: f64,
    y: f64,
}

#[export(wstp)]
fn create_point2(args: Vec<Expr>) -> Expr {
    assert!(args.is_empty());

    let point = Point { x: 3.0, y: 4.0 };

    point.to_expr()
}

impl Point {
    fn to_expr(&self) -> Expr {
        let Point { x, y } = *self;

        expr!(System::Point[System::List[x, y]])
    }
}
