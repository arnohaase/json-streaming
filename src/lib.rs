#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "blocking")]
#[allow(dead_code)]
pub mod blocking;

#[allow(dead_code)]
pub mod shared;

#[cfg(feature = "non-blocking")]
#[allow(dead_code)]
pub mod nonblocking;


//TODO examples
