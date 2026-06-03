//! Shared helpers: attribute parsing, name resolution, error helpers.
//!
//! The container/variant attribute `#[wolfram(symbol = "MyPkg`Foo")]` overrides
//! the default `Global`Name` qualification; the field attribute
//! `#[wolfram(rename = "fieldName")]` overrides the default snake_case key
//! used in Association entries.

use syn::{Attribute, Error, Lit, Meta, NestedMeta, Result};

/// Container/variant-level attributes parsed from `#[wolfram(...)]`.
#[derive(Default)]
pub(crate) struct ContainerAttrs {
    /// Override for the symbol/head used to identify this struct or variant on
    /// the wire (e.g. `"MyPkg`Foo"`). When `None`, the macro qualifies the
    /// Rust ident with `Global`` automatically.
    pub symbol: Option<String>,
    /// Override for the List head used to wrap enum variants on the wire.
    /// Defaults to `"System`List"`. Use `#[wolfram(enum_head = "System`Failure")]`
    /// to emit `Failure["VariantName", <|fields|>]` instead of `{"VariantName", ...}`.
    pub enum_head: Option<String>,
    /// Transform applied to every Association key that lacks an explicit
    /// `#[wolfram(rename = "...")]`. `None` (default) leaves keys verbatim;
    /// `"snake_to_camelcase"` upper-camel-cases them (`out_of_range` → `OutOfRange`).
    /// `WxfError` defaults this to `"snake_to_camelcase"` so failures get
    /// Wolfram-style keys.
    pub key_processor: Option<String>,
}

/// Apply a `key_processor` policy to one Association key. `None` (or an unknown
/// policy) leaves the key untouched; `"snake_to_camelcase"` upper-camel-cases each
/// `_`-separated segment (`message_template` → `MessageTemplate`).
pub(crate) fn process_key(key: &str, processor: Option<&str>) -> String {
    match processor {
        Some("snake_to_camelcase") => key
            .split('_')
            .map(|seg| {
                let mut chars = seg.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + chars.as_str()
                    },
                    None => String::new(),
                }
            })
            .collect(),
        _ => key.to_string(),
    }
}

/// Field-level attributes parsed from `#[wolfram(...)]`.
#[derive(Default)]
pub(crate) struct FieldAttrs {
    /// Override for the Association key used to identify this field on the
    /// wire. When `None`, the macro uses the field's Rust ident verbatim.
    pub rename: Option<String>,
}

pub(crate) fn parse_container_attrs(attrs: &[Attribute]) -> Result<ContainerAttrs> {
    let mut out = ContainerAttrs::default();
    for attr in attrs {
        if !attr.path.is_ident("wolfram") {
            continue;
        }
        let meta = attr.parse_meta()?;
        let list = match meta {
            Meta::List(list) => list,
            _ => return Err(Error::new_spanned(attr, "expected `#[wolfram(...)]`")),
        };
        for nested in list.nested {
            match nested {
                NestedMeta::Meta(Meta::NameValue(nv)) if nv.path.is_ident("symbol") => {
                    out.symbol = Some(string_lit(&nv.lit)?);
                },
                NestedMeta::Meta(Meta::NameValue(nv))
                    if nv.path.is_ident("enum_head") =>
                {
                    out.enum_head = Some(string_lit(&nv.lit)?);
                },
                NestedMeta::Meta(Meta::NameValue(nv))
                    if nv.path.is_ident("key_processor") =>
                {
                    out.key_processor = Some(string_lit(&nv.lit)?);
                },
                other => {
                    return Err(Error::new_spanned(
                        other,
                        "unknown `#[wolfram(...)]` option; expected `symbol`, `enum_head`, or `key_processor`",
                    ));
                },
            }
        }
    }
    Ok(out)
}

pub(crate) fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    for attr in attrs {
        if !attr.path.is_ident("wolfram") {
            continue;
        }
        let meta = attr.parse_meta()?;
        let list = match meta {
            Meta::List(list) => list,
            _ => return Err(Error::new_spanned(attr, "expected `#[wolfram(...)]`")),
        };
        for nested in list.nested {
            match nested {
                NestedMeta::Meta(Meta::NameValue(nv)) if nv.path.is_ident("rename") => {
                    out.rename = Some(string_lit(&nv.lit)?);
                },
                other => {
                    return Err(Error::new_spanned(
                        other,
                        "unknown `#[wolfram(...)]` option here; expected `rename = \"...\"`",
                    ));
                },
            }
        }
    }
    Ok(out)
}

fn string_lit(lit: &Lit) -> Result<String> {
    match lit {
        Lit::Str(s) => Ok(s.value()),
        other => Err(Error::new_spanned(other, "expected a string literal")),
    }
}

/// Apply the default `Global`` context to a bare Rust ident if the user did
/// not override via `#[wolfram(symbol = "...")]`. The result is a fully-
/// qualified WL symbol name suitable for `Symbol::try_from_wxf_name_owned`.
pub(crate) fn qualify_symbol(ident_str: &str, container: &ContainerAttrs) -> String {
    container
        .symbol
        .clone()
        .unwrap_or_else(|| format!("Global`{}", ident_str))
}
