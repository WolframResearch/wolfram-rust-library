//! [`ViaWXF`] — bridge a foreign type to WXF through an intermediate
//! representation.
//!
//! **Crate-internal only.** This is implementation plumbing for the `vendor`
//! bridges (`vendor::chrono`, `vendor::num_bigint`, `vendor::num_complex`,
//! `vendor::chrono_tz`) — not part of this crate's public API. Downstream
//! crates can't add their own `ViaWXF` bridges; the vendored types this crate
//! bridges are foreign to `wolfram-serialize` too, so there's nothing
//! `pub`-specific about the mechanism itself.
//!
//! Some types (third-party "vendor" types especially) have a natural Wolfram
//! Language expression shape but can't sensibly derive [`ToWXF`]/[`FromWXF`]
//! themselves — their fields don't mirror the wire form. The pattern here is
//! to declare a plain Rust struct that *does* mirror the wire form, derive the
//! WXF traits on it, and implement [`ViaWXF`] to convert between the two —
//! see e.g. `vendor::num_complex` for the smallest real example. Sketch:
//!
//! ```ignore
//! struct Celsius(f64); // pretend this is a foreign type
//!
//! /// Wire form: `Quantity[magnitude, "DegreesCelsius"]`.
//! #[derive(ToWXF, FromWXF)]
//! #[wolfram(symbol = "System`Quantity")]
//! struct Quantity {
//!     magnitude: f64,
//!     unit: String,
//! }
//!
//! impl ViaWXF for Celsius {
//!     type Via = Quantity;
//!     fn to_via(&self) -> Quantity {
//!         Quantity { magnitude: self.0, unit: "DegreesCelsius".into() }
//!     }
//!     fn from_via(via: Quantity) -> Result<Celsius, Error> {
//!         if via.unit != "DegreesCelsius" {
//!             return Err(Error::invalid(format!("unexpected unit {:?}", via.unit)));
//!         }
//!         Ok(Celsius(via.magnitude))
//!     }
//! }
//! impl_via_wxf!(Celsius);
//! ```
//!
//! A blanket `impl<T: ViaWXF> ToWXF for T` would conflict (coherence) with the
//! crate's concrete `ToWXF` impls, so the delegating impls are generated per
//! type by [`impl_via_wxf!`].

use crate::from_wxf::FromWXF;
use crate::to_wxf::ToWXF;
use crate::Error;

/// Bridge to WXF through an intermediate representation ([`Self::Via`]) that
/// implements the WXF traits — typically via `#[derive(ToWXF, FromWXF)]`.
///
/// Pair every impl with an [`impl_via_wxf!`] invocation, which generates the
/// [`ToWXF`]/[`FromWXF`] impls that delegate through this trait.
pub(crate) trait ViaWXF: Sized {
    /// The wire-shaped intermediate representation.
    type Via: ToWXF + for<'de> FromWXF<'de>;

    /// Convert to the intermediate representation (infallible — every value of
    /// `Self` has a wire form).
    fn to_via(&self) -> Self::Via;

    /// Convert back from the intermediate representation. Fallible: the wire
    /// form may carry values with no `Self` counterpart (e.g. an out-of-range
    /// date, a negative unsigned integer).
    fn from_via(via: Self::Via) -> Result<Self, Error>;
}

/// Generate delegating [`ToWXF`] and [`FromWXF`] impls for one or more types
/// that implement [`ViaWXF`]. See the [module docs][self] for a full example.
/// Crate-internal — re-exported at the crate root as `pub(crate)` by `lib.rs`.
macro_rules! impl_via_wxf {
    ($($ty:ty),+ $(,)?) => {$(
        impl $crate::ToWXF for $ty {
            fn to_wxf<W: $crate::Writer>(
                &self,
                w: &mut $crate::WxfWriter<W>,
            ) -> ::core::result::Result<(), $crate::Error> {
                $crate::ToWXF::to_wxf(&$crate::via::ViaWXF::to_via(self), w)
            }
        }

        impl<'de> $crate::FromWXF<'de> for $ty {
            fn from_wxf_with_tag<R: $crate::Reader<'de>>(
                r: &mut $crate::WxfReader<R>,
                tok: $crate::ExpressionEnum,
            ) -> ::core::result::Result<Self, $crate::Error> {
                let via = <<$ty as $crate::via::ViaWXF>::Via as $crate::FromWXF<'de>>::from_wxf_with_tag(r, tok)?;
                <$ty as $crate::via::ViaWXF>::from_via(via)
            }
        }
    )+};
}
pub(crate) use impl_via_wxf;
