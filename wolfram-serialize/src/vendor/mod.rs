//! Optional WXF conversions for third-party ("vendor") crate types.
//!
//! Each submodule bridges one vendor crate to a Wolfram Language expression
//! shape via the `ViaWXF` pattern: a plain Rust struct that
//! mirrors the wire form derives `ToWXF`/`FromWXF`, and the vendor type
//! converts to/from it. Every submodule is only compiled when the matching
//! `vendor-<crate>` Cargo feature is enabled.

#[cfg(feature = "vendor-chrono")]
pub mod chrono;

// Named IANA time zones (chrono-tz vendors the whole time zone database, so
// this is separate from the base `vendor-chrono` bridge).
#[cfg(feature = "vendor-chrono-tz")]
pub mod chrono_tz;

#[cfg(feature = "vendor-num-bigint")]
pub mod num_bigint;

#[cfg(feature = "vendor-num-complex")]
pub mod num_complex;
