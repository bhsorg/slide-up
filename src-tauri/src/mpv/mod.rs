pub mod client;
pub mod ffi;
#[cfg(windows)]
pub mod native;

pub use client::{EndReason, MpvClient};
