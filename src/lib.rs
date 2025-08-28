#![cfg_attr(feature = "no-std", no_std)]

pub mod blocking;
pub mod format;
pub mod nonblocking;


//TODO top-level pub use
//TODO feature flag for async / blocking support
//TODO object-per-line
//TODO unit test for pluggable FloatFormat
