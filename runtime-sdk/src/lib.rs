//! Oasis runtime SDK.
#![feature(const_fn)]
#![deny(rust_2018_idioms, unreachable_pub)]
#![forbid(unsafe_code)]

pub mod context;
pub mod crypto;
pub mod dispatcher;
pub mod error;
pub mod event;
pub mod module;
pub mod modules;
pub mod runtime;
pub mod storage;
pub mod testing;
pub mod types;

pub use crate::{
    context::{DispatchContext, TxContext},
    module::Module,
    runtime::Runtime,
};

// Re-export the appropriate version of the oasis-core-runtime library.
pub use oasis_core_runtime as core;

// Re-export the SDK support proc-macros.
#[cfg(feature = "oasis-runtime-sdk-macros")]
pub use oasis_runtime_sdk_macros::*;
