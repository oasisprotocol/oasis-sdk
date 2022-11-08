//! Oasis runtime SDK.
#![feature(test)]
#![deny(rust_2018_idioms, unreachable_pub)]
#![forbid(unsafe_code)]
#![feature(int_log)]

pub mod callformat;
pub mod config;
pub mod context;
pub mod crypto;
pub mod dispatcher;
pub mod error;
pub mod event;
pub mod keymanager;
pub mod module;
pub mod modules;
pub mod runtime;
pub mod schedule_control;
pub mod sender;
pub mod storage;
pub mod testing;
pub mod types;

pub use crate::{
    context::{BatchContext, Context, TxContext},
    core::common::version::Version,
    module::Module,
    runtime::Runtime,
};

// Re-export the appropriate version of the oasis-core-runtime library.
pub use oasis_core_runtime as core;

// Re-export the cbor crate.
pub use cbor;

// Re-export the SDK support proc-macros.
#[cfg(feature = "oasis-runtime-sdk-macros")]
pub use oasis_runtime_sdk_macros::*;

// Required so that proc-macros can refer to items within this crate.
use crate as oasis_runtime_sdk;
