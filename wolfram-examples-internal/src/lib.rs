pub mod core;

mod legacy_native;
#[cfg(feature = "wstp")]
mod legacy_wstp;
mod margs;
mod mem;
mod native;
#[cfg(feature = "wstp")]
mod wstp;
mod wxf;
