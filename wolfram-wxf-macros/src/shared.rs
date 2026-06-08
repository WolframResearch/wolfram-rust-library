//! Shared helpers: attribute parsing, name resolution, error helpers.
//!
//! The container/variant attribute `#[wolfram(symbol = "MyPkg`Foo")]` overrides
//! the bare Rust ident name used for a unit struct/variant symbol (no context is
//! imposed otherwise); the field attribute `#[wolfram(rename = "fieldName")]`
//! overrides the default snake_case key used in Association entries.

use syn::{Attribute, Error, Lit, Meta, NestedMeta, Result};

/// The WL head used to wrap an enum variant on the wire. Set at the container
/// (default for all variants) and/or per variant (override).
#[derive(Default, Clone)]
pub(crate) enum EnumHead {
    /// Not specified here — inherit the container default (ultimately `System`List`).
    #[default]
    Unset,
    /// `#[wolfram(enum_head = false)]` — *transparent*: no head and no variant tag;
    /// the variant's single payload is serialized directly.
    Transparent,
    /// `#[wolfram(enum_head = "Sym")]` — wrap as `Sym["Variant", …]`.
    Head(String),
}

/// Container/variant-level attributes parsed from `#[wolfram(...)]`.
#[derive(Default)]
pub(crate) struct ContainerAttrs {
    /// Override for the symbol/head used to identify this struct or variant on
    /// the wire (e.g. `"MyPkg`Foo"`). When `None`, the bare Rust ident name is
    /// used verbatim — no context is imposed.
    pub symbol: Option<String>,
    /// Head used to wrap enum variants. Defaults to `System`List`;
    /// `#[wolfram(enum_head = "System`Failure")]` emits `Failure["Variant", …]`,
    /// `#[wolfram(enum_head = false)]` emits the bare payload (see [`EnumHead`]).
    pub enum_head: EnumHead,
    /// Transform applied to every Association key that lacks an explicit
    /// `#[wolfram(rename = "...")]`. `None` (default) leaves keys verbatim;
    /// `"CamelCase"` upper-camel-cases them (`out_of_range` → `OutOfRange`).
    /// Pair with `enum_head = "System`Failure"` to get Wolfram-style failure keys.
    ///
    /// Must be a compile-time-known name, not a function path: keys are baked into
    /// the generated code as string literals, and the deserialize side matches them
    /// in `match key { ... }` arms (patterns must be constants), so a runtime
    /// `fn(&str) -> String` can't be used.
    pub key_processor: Option<String>,
}

/// Apply a `key_processor` policy to one Association key. `None` (or an unknown
/// policy) leaves the key untouched; `"CamelCase"` upper-camel-cases each
/// `_`-separated segment (`message_template` → `MessageTemplate`).
pub(crate) fn process_key(key: &str, processor: Option<&str>) -> String {
    match processor {
        Some("CamelCase") => key
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
                    out.enum_head = match &nv.lit {
                        Lit::Str(s) => EnumHead::Head(s.value()),
                        Lit::Bool(b) if !b.value => EnumHead::Transparent,
                        other => {
                            return Err(Error::new_spanned(
                                other,
                                "enum_head expects a head string (e.g. \"System`Failure\") or `false` for no head",
                            ))
                        },
                    };
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
