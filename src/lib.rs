#![cfg_attr(feature = "no-std", no_std)]

#[cfg(feature = "blocking")]
pub mod blocking;
pub mod shared;
#[cfg(feature = "non-blocking")]
pub mod nonblocking;


//TODO object-per-line
