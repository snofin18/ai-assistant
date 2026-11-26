//! CDP runtime 通道。

pub mod errors;
mod http;
mod json;
#[cfg(test)]
pub(crate) use json::{JsonValue, parse};
pub mod runtime;
mod websocket;
