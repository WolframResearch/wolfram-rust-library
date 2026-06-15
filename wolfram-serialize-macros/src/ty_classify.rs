//! Classify a `syn::Type` to decide which WXF emission path applies at a field
//! site. Only types whose wire shape varies with their element type need a
//! dedicated `FieldKind` variant — anything else falls through to `Other` and
//! the derive emits a delegated call through the `ToWolfram` trait.
//!
//! Numeric tensor detection (rectangular-homogeneous nested tuples / fixed-
//! size arrays of a single numeric primitive) lives here because it requires
//! recursive structural analysis at macro-expansion time.

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{GenericArgument, PathArguments, Type, TypeArray, TypePath, TypeTuple};

/// What the derive should emit for a given field type.
pub(crate) enum FieldKind<'a> {
    /// `Vec<u8>` → `serialize_byte_array(&self.field)`
    VecOfU8,
    /// `Vec<T>` for a numeric `T` (i8..i64, u16..u64, f32, f64).
    /// `dt` is a tokenstream for the `NumericArrayDataType` constant; `elem_ty`
    /// is the original primitive type.
    VecOfNumeric { elem_ty: &'a Type, dt: TokenStream },
    /// `Vec<T>` for any other `T` — emit a `Function[List, ...]` element-by-element.
    VecOfOther { elem_ty: &'a Type },
    /// Rectangular-homogeneous numeric tensor — built from a tuple, a `[T;N]`,
    /// or a recursive nest of those. `elem_ty` is the leaf primitive,
    /// `dt` is a tokenstream for the `NumericArrayDataType` constant,
    /// `dims` is the full multi-dim shape of the tensor, and
    /// `flat_indices` enumerates the field accessors needed to read every leaf
    /// in row-major order (e.g. for `((f64, f64), (f64, f64))` it would be
    /// `["0.0", "0.1", "1.0", "1.1"]`). For `[T; N]`-rooted tensors the
    /// list is empty and the byte cast applies directly.
    NumericTensor {
        elem_ty: &'a Type,
        dt: TokenStream,
        dims: Vec<usize>,
        /// `Some(_)`: tuple-rooted; the strings are dot-separated index paths
        /// (e.g. `"0.1"`) into the tuple to read each leaf in row-major order,
        /// requiring a stack `[T; N]` copy-trampoline before the byte cast.
        /// `None`: array-rooted (`[T; N]` or `[[T; M]; N]` etc.); the field
        /// is already laid out contiguously and a direct byte cast is sound.
        tuple_paths: Option<Vec<String>>,
        /// Original Rust type — used for span-attaching generated code.
        original_ty: &'a Type,
    },
    /// Heterogeneous tuple — emit a `Function[List, ...]` of fields.
    TupleHetero { tup: &'a TypeTuple },
    /// Fixed-size array `[T; N]` whose `T` isn't a numeric primitive — emit a
    /// `Function[List, ...]` of `N` elements.
    ArrayHetero { arr: &'a TypeArray, len: usize },
    /// Anything else — delegate via `<Ty as ToWolfram>::serialize(&field, s)`.
    Other,
}

pub(crate) fn classify(ty: &Type) -> FieldKind<'_> {
    // Vec<...> — most common.
    if let Some(elem) = vec_inner(ty) {
        return classify_vec(elem);
    }
    // [T; N]
    if let Type::Array(arr) = ty {
        if let Some(len) = array_len(&arr.len) {
            if let Some(tensor) = numeric_tensor_of(ty) {
                return tensor;
            }
            return FieldKind::ArrayHetero { arr, len };
        }
    }
    // (...) tuple
    if let Type::Tuple(tup) = ty {
        if !tup.elems.is_empty() {
            if let Some(tensor) = numeric_tensor_of(ty) {
                return tensor;
            }
            return FieldKind::TupleHetero { tup };
        }
    }
    FieldKind::Other
}

fn classify_vec(elem: &Type) -> FieldKind<'_> {
    if let Some(prim) = primitive_ident_of(elem) {
        if prim == "u8" {
            return FieldKind::VecOfU8;
        }
        if let Some(dt) = numeric_dt_for(&prim) {
            return FieldKind::VecOfNumeric { elem_ty: elem, dt };
        }
    }
    FieldKind::VecOfOther { elem_ty: elem }
}

/// If `ty` is a rectangular-homogeneous nested numeric tensor (built from
/// pure-tuple OR pure-array nesting — not mixed), return its tensor
/// description. Mixed tuple/array nests would need a more elaborate
/// flattening scheme; v1 keeps the two paths clean and simple.
fn numeric_tensor_of(ty: &Type) -> Option<FieldKind<'_>> {
    // Try pure-array nest first (e.g. [[f64; 3]; 2]); falls back to pure-
    // tuple nest if the root isn't an array.
    if let Type::Array(_) = ty {
        let (leaf, dims) = peel_array_nest(ty)?;
        let prim = primitive_ident_of(leaf)?;
        let dt = numeric_dt_for(&prim)?;
        return Some(FieldKind::NumericTensor {
            elem_ty: leaf,
            dt,
            dims,
            tuple_paths: None,
            original_ty: ty,
        });
    }
    if let Type::Tuple(_) = ty {
        let (leaf, dims) = peel_tuple_nest(ty)?;
        let prim = primitive_ident_of(leaf)?;
        let dt = numeric_dt_for(&prim)?;
        let paths = generate_tuple_paths(ty);
        return Some(FieldKind::NumericTensor {
            elem_ty: leaf,
            dt,
            dims,
            tuple_paths: Some(paths),
            original_ty: ty,
        });
    }
    None
}

/// Walk only `Type::Array` layers; bail out on tuples or anything else (we
/// only call this when the root is already an Array). Returns the leaf type
/// and the dimension shape outermost-to-innermost.
fn peel_array_nest(ty: &Type) -> Option<(&Type, Vec<usize>)> {
    match ty {
        Type::Array(arr) => {
            let n = array_len(&arr.len)?;
            let (leaf, mut inner_dims) = peel_array_nest(&arr.elem)?;
            let mut dims = vec![n];
            dims.append(&mut inner_dims);
            Some((leaf, dims))
        },
        // Leaf — must not be a Tuple (mixed nests are rejected for v1).
        Type::Tuple(_) => None,
        _ => Some((ty, Vec::new())),
    }
}

/// Walk only `Type::Tuple` layers; require homogeneity (all elements same
/// type with same nested shape). Bail on arrays or anything else inside.
fn peel_tuple_nest(ty: &Type) -> Option<(&Type, Vec<usize>)> {
    match ty {
        Type::Tuple(tup) => {
            let n = tup.elems.len();
            if n == 0 {
                return None;
            }
            let mut iter = tup.elems.iter();
            let first = iter.next().unwrap();
            let (leaf, first_inner_dims) = peel_tuple_nest(first)?;
            for next in iter {
                let (other_leaf, other_inner_dims) = peel_tuple_nest(next)?;
                if !same_type(leaf, other_leaf) || other_inner_dims != first_inner_dims {
                    return None;
                }
            }
            let mut dims = vec![n];
            dims.extend(first_inner_dims);
            Some((leaf, dims))
        },
        // Leaf — must not be an Array (mixed nests are rejected for v1).
        Type::Array(_) => None,
        _ => Some((ty, Vec::new())),
    }
}

/// Build the row-major list of dot-separated index paths into a tuple-rooted
/// tensor. Each path is a string like `"0.1.2"` that the caller can splice
/// after the field accessor (e.g. `self.field.0.1.2`).
fn generate_tuple_paths(ty: &Type) -> Vec<String> {
    fn walk(ty: &Type, prefix: &str, out: &mut Vec<String>) {
        match ty {
            Type::Tuple(tup) => {
                for (i, elem) in tup.elems.iter().enumerate() {
                    let next = if prefix.is_empty() {
                        i.to_string()
                    } else {
                        format!("{}.{}", prefix, i)
                    };
                    walk(elem, &next, out);
                }
            },
            _ => {
                // Leaf reached — record the accumulated path. (For pure-tuple
                // nests the leaf is always a primitive type.)
                if !prefix.is_empty() {
                    out.push(prefix.to_string());
                }
            },
        }
    }
    let mut out = Vec::new();
    walk(ty, "", &mut out);
    out
}

/// Returns the inner type of `Vec<T>`, or `None` if not a `Vec`.
fn vec_inner(ty: &Type) -> Option<&Type> {
    let path = match ty {
        Type::Path(TypePath { qself: None, path }) => path,
        _ => return None,
    };
    let last = path.segments.last()?;
    if last.ident != "Vec" {
        return None;
    }
    let args = match &last.arguments {
        PathArguments::AngleBracketed(a) => a,
        _ => return None,
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    }
}

/// Returns the primitive identifier (e.g. `"i32"`) iff `ty` is one of the 10
/// numeric primitives (`i8..i64`, `u8..u64`, `f32`, `f64`). Used by the
/// deserialize derive to detect "all numeric same-typed fields" — structs
/// that can also accept a single NumericArray/PackedArray/ByteArray payload.
pub(crate) fn numeric_primitive_name(ty: &Type) -> Option<String> {
    let prim = primitive_ident_of(ty)?;
    match prim.as_str() {
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => {
            Some(prim)
        },
        _ => None,
    }
}

/// Returns the bare primitive identifier (e.g. "i32") if `ty` is a path with
/// a single-segment identifier in the numeric primitive set.
fn primitive_ident_of(ty: &Type) -> Option<String> {
    let path = match ty {
        Type::Path(TypePath { qself: None, path }) => path,
        _ => return None,
    };
    if path.segments.len() != 1 {
        return None;
    }
    Some(path.segments[0].ident.to_string())
}

/// `NumericArrayDataType::*` token for a primitive ident, or `None` if the
/// primitive is not a NumericArray element type.
fn numeric_dt_for(prim: &str) -> Option<TokenStream> {
    let variant = match prim {
        "i8" => "Integer8",
        "i16" => "Integer16",
        "i32" => "Integer32",
        "i64" => "Integer64",
        "u8" => "UnsignedInteger8",
        "u16" => "UnsignedInteger16",
        "u32" => "UnsignedInteger32",
        "u64" => "UnsignedInteger64",
        "f32" => "Real32",
        "f64" => "Real64",
        _ => return None,
    };
    let ident = syn::Ident::new(variant, proc_macro2::Span::call_site());
    Some(quote! { ::wolfram_serialize::NumericArrayEnum::#ident })
}

fn array_len(expr: &syn::Expr) -> Option<usize> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(li),
        ..
    }) = expr
    {
        li.base10_parse::<usize>().ok()
    } else {
        None
    }
}

fn same_type(a: &Type, b: &Type) -> bool {
    a.to_token_stream().to_string() == b.to_token_stream().to_string()
}

/// Returns `true` if `ty` is syntactically `Option<…>` (any path ending in
/// the segment `Option` with one generic arg). Used by the deserialize derive
/// to make absent keys default to `None` instead of erroring.
pub(crate) fn is_option_type(ty: &Type) -> bool {
    let path = match ty {
        Type::Path(TypePath { qself: None, path }) => path,
        _ => return false,
    };
    let last = match path.segments.last() {
        Some(s) => s,
        None => return false,
    };
    if last.ident != "Option" {
        return false;
    }
    matches!(&last.arguments, PathArguments::AngleBracketed(args) if args.args.len() == 1)
}
