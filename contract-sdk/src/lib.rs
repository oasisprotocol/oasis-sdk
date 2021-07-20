//! Oasis Contract SDK.
#![cfg_attr(target_arch = "wasm32", feature(wasm_abi))]

#[cfg(target_arch = "wasm32")]
pub mod abi;
pub mod context;
pub mod contract;
pub mod env;
pub mod error;
pub mod event;
pub mod memory;
pub mod storage;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;

// Re-export types.
pub use oasis_contract_sdk_types as types;

// Re-export the CBOR crate for use in macros.
pub use cbor;

// Re-exports.
pub use self::{context::Context, contract::Contract, error::Error, event::Event};

// Re-export the SDK support proc-macros.
#[cfg(feature = "oasis-contract-sdk-macros")]
pub use oasis_contract_sdk_macros::*;

// Use `wee_alloc` as the global allocator.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
