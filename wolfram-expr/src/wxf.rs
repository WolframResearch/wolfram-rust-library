//! WXF wire-format enums, re-exported from [`wolfram_serialize`], plus the
//! [`ArrayElement`] impls that map Rust primitives to element-type tags.
//!
//! The enum definitions live in the dependency-free `wolfram-serialize` crate; they
//! are re-exported here so existing paths (`wolfram_expr::wxf::ExpressionEnum`,
//! `wolfram_expr::NumericArrayEnum`, …) keep resolving.

#![allow(missing_docs)]

pub use wolfram_serialize::constants::{
    ExpressionEnum, HeaderEnum, NumericArrayEnum, PackedArrayEnum,
};

use crate::array_buf::ArrayElement;
use crate::complex::{Complex32, Complex64};

//======================================
// ArrayElement impls — Rust primitive → enum variant
// Single source of truth for both NumericArrayEnum and PackedArrayEnum.
//======================================

impl ArrayElement<NumericArrayEnum> for i8 {
    const TAG: NumericArrayEnum = NumericArrayEnum::Integer8;
}
impl ArrayElement<NumericArrayEnum> for i16 {
    const TAG: NumericArrayEnum = NumericArrayEnum::Integer16;
}
impl ArrayElement<NumericArrayEnum> for i32 {
    const TAG: NumericArrayEnum = NumericArrayEnum::Integer32;
}
impl ArrayElement<NumericArrayEnum> for i64 {
    const TAG: NumericArrayEnum = NumericArrayEnum::Integer64;
}
impl ArrayElement<NumericArrayEnum> for u8 {
    const TAG: NumericArrayEnum = NumericArrayEnum::UnsignedInteger8;
}
impl ArrayElement<NumericArrayEnum> for u16 {
    const TAG: NumericArrayEnum = NumericArrayEnum::UnsignedInteger16;
}
impl ArrayElement<NumericArrayEnum> for u32 {
    const TAG: NumericArrayEnum = NumericArrayEnum::UnsignedInteger32;
}
impl ArrayElement<NumericArrayEnum> for u64 {
    const TAG: NumericArrayEnum = NumericArrayEnum::UnsignedInteger64;
}
impl ArrayElement<NumericArrayEnum> for f32 {
    const TAG: NumericArrayEnum = NumericArrayEnum::Real32;
}
impl ArrayElement<NumericArrayEnum> for f64 {
    const TAG: NumericArrayEnum = NumericArrayEnum::Real64;
}
impl ArrayElement<NumericArrayEnum> for Complex32 {
    const TAG: NumericArrayEnum = NumericArrayEnum::ComplexReal32;
}
impl ArrayElement<NumericArrayEnum> for Complex64 {
    const TAG: NumericArrayEnum = NumericArrayEnum::ComplexReal64;
}

impl ArrayElement<PackedArrayEnum> for i8 {
    const TAG: PackedArrayEnum = PackedArrayEnum::Integer8;
}
impl ArrayElement<PackedArrayEnum> for i16 {
    const TAG: PackedArrayEnum = PackedArrayEnum::Integer16;
}
impl ArrayElement<PackedArrayEnum> for i32 {
    const TAG: PackedArrayEnum = PackedArrayEnum::Integer32;
}
impl ArrayElement<PackedArrayEnum> for i64 {
    const TAG: PackedArrayEnum = PackedArrayEnum::Integer64;
}
impl ArrayElement<PackedArrayEnum> for f32 {
    const TAG: PackedArrayEnum = PackedArrayEnum::Real32;
}
impl ArrayElement<PackedArrayEnum> for f64 {
    const TAG: PackedArrayEnum = PackedArrayEnum::Real64;
}
impl ArrayElement<PackedArrayEnum> for Complex32 {
    const TAG: PackedArrayEnum = PackedArrayEnum::ComplexReal32;
}
impl ArrayElement<PackedArrayEnum> for Complex64 {
    const TAG: PackedArrayEnum = PackedArrayEnum::ComplexReal64;
}
