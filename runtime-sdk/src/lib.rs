//! Oasis runtime SDK.
#![feature(const_fn)]

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
