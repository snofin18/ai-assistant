//! Dedicated browser profiles for CDP-capable Edge/Chrome.

#![deny(unsafe_code)]

mod error;
mod profile;

pub use error::{BrowserProfileError, BrowserProfileResult};
pub use profile::{BrowserKind, BrowserProfile};
