//! `#[wolfram(enum_head = false)]` is transparent: the variant serializes as
//! just its single payload — no head, no variant tag — while sibling variants
//! keep their (per-variant) heads.

use wolfram_expr::{expr, from_wxf, to_wxf, Expr, ToWXF};

#[derive(ToWXF)]
#[wolfram(enum_head = "System`Failure", key_processor = "CamelCase")]
enum Outcome {
    #[wolfram(enum_head = "System`Success")]
    Ok(i64),
    /// Transparent — serializes as the bare payload.
    #[wolfram(enum_head = false)]
    Raw(Expr),
    Bad {
        message: String,
    },
}

fn rt<T: ToWXF>(v: &T) -> Expr {
    from_wxf::<Expr>(&to_wxf(v, None).expect("serialize")).expect("parse")
}

#[test]
fn transparent_variant_serializes_payload_directly() {
    // The whole enum serializes as just the inner Expr — no Success/Failure/tag.
    assert_eq!(rt(&Outcome::Raw(Expr::from(7i64))), expr!(7));
    assert_eq!(
        rt(&Outcome::Raw(expr!(System::List[1, 2, 3]))),
        expr!(System::List[1, 2, 3])
    );
}

#[test]
fn sibling_variants_keep_their_heads() {
    assert_eq!(rt(&Outcome::Ok(7)), expr!(System::Success["Ok", 7]));
    assert_eq!(
        rt(&Outcome::Bad {
            message: "boom".into()
        }),
        expr!(System::Failure["Bad", {"Message" -> "boom"}])
    );
}
