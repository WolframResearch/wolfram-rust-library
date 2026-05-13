//! `#[doc(hidden)]` helpers used by code generated from `#[derive(ToWolfram)]`
//! and `#[derive(FromWolfram)]`. Not part of the public API — names and
//! signatures here may change between versions.
//!
//! The thunk types let the derive feed the existing
//! [`Serializer::serialize_association`][crate::Serializer::serialize_association]
//! API, which expects each entry as a `(&dyn ToWolfram, &dyn ToWolfram, bool)`,
//! without per-derive boilerplate types.

use std::any::TypeId;

use crate::serializer::ToWolfram;
use crate::{Error, Serializer};
use wolfram_expr::NumericArrayDataType;

/// Wraps a borrowed byte slice as a `ToWolfram` whose serialize call emits a
/// `ByteArray` token. Used by the derive when a struct field is `Vec<u8>` or
/// `[u8; N]`, to slot it into an Association entry.
pub struct ByteArrayThunk<'a>(pub &'a [u8]);

impl<'a> ToWolfram for ByteArrayThunk<'a> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_byte_array(self.0)
    }
}

/// Wraps a typed numeric byte slice + data-type tag + dimensions as a
/// `ToWolfram` whose serialize call emits a `NumericArray` token. Used by
/// the derive when a struct field maps to a 1-D or k-D `NumericArray`.
pub struct NumericArrayThunk<'a> {
    /// Element-type tag (e.g. `Integer32`).
    pub data_type: NumericArrayDataType,
    /// Dimension shape (row-major).
    pub dimensions: &'a [usize],
    /// Flat row-major little-endian byte buffer.
    pub bytes: &'a [u8],
}

impl<'a> ToWolfram for NumericArrayThunk<'a> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_numeric_array(self.data_type, self.dimensions, self.bytes)
    }
}

/// Wraps a slice of `&dyn ToWolfram` references as a `ToWolfram` that emits
/// `Function[List, …]`. Used by the derive when a field is a heterogeneous
/// tuple, a fixed-size array of non-numeric `T`, or a `Vec<T_other>` (where
/// the macro built up a `Vec<&dyn ToWolfram>` of element refs).
pub struct ListThunk<'a>(pub &'a [&'a dyn ToWolfram]);

impl<'a> ToWolfram for ListThunk<'a> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        let head: &dyn ToWolfram = &SystemListSymbol;
        s.serialize_function(head, self.0)
    }
}

/// Helper type whose `ToWolfram` impl just emits `System`List`. Lets
/// [`ListThunk`] feed a `&dyn ToWolfram` head into `serialize_function`.
struct SystemListSymbol;
impl ToWolfram for SystemListSymbol {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_symbol("System`List")
    }
}

/// Wraps an arbitrary symbol name string for use as a `Function` head. Used
/// by the derive when emitting `Function[Symbol("Global`StructName"), …]`
/// for tuple structs, enum tuple variants, and enum struct variants.
pub struct HeadSymbol(pub &'static str);

impl ToWolfram for HeadSymbol {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_symbol(self.0)
    }
}

/// Wraps a slice of pre-built association entries as a `ToWolfram` that emits
/// an `Association` token. Used by the derive for enum struct-variants where
/// the inner Association needs to be wrapped in `Function[Symbol(VariantName), …]`.
pub struct AssocThunk<'a>(pub &'a [(&'a dyn ToWolfram, &'a dyn ToWolfram, bool)]);

impl<'a> ToWolfram for AssocThunk<'a> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_association(self.0)
    }
}

/// Debug-only one-shot `eprintln!` for the case where the derive's generic-
/// `Vec<T>` fallback is instantiated with a numeric primitive — meaning the
/// user is paying List overhead instead of NumericArray. Compiled out in
/// release builds (the macro guards the call with `#[cfg(debug_assertions)]`).
#[cfg(debug_assertions)]
pub fn warn_if_numeric_in_list<T: 'static + ?Sized>(file: &str, line: u32, field: &str) {
    let t = TypeId::of::<T>();
    let is_numeric = t == TypeId::of::<i8>()
        || t == TypeId::of::<i16>()
        || t == TypeId::of::<i32>()
        || t == TypeId::of::<i64>()
        || t == TypeId::of::<u8>()
        || t == TypeId::of::<u16>()
        || t == TypeId::of::<u32>()
        || t == TypeId::of::<u64>()
        || t == TypeId::of::<f32>()
        || t == TypeId::of::<f64>();
    if is_numeric {
        eprintln!(
            "[wolfram-serializer] {}:{}: field {:?} is a generic Vec<T> instantiated \
             with a numeric primitive — serialized as List[…] instead of NumericArray. \
             Use a concrete `Vec<{}>` field or wrap explicitly in NumericArray to avoid \
             the per-element overhead.",
            file,
            line,
            field,
            std::any::type_name::<T>()
        );
    }
}

/// Release-build no-op (matches the debug signature so generated code can call
/// it unconditionally without a `cfg!`).
#[cfg(not(debug_assertions))]
pub fn warn_if_numeric_in_list<T: 'static + ?Sized>(
    _file: &str,
    _line: u32,
    _field: &str,
) {
}
