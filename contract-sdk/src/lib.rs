//! Oasis Contract SDK.
#![feature(wasm_abi)]

pub mod context;
pub mod contract;
#[cfg(target_arch = "wasm32")]
pub mod exports;
pub mod memory;

// Re-export types.
pub use oasis_contract_sdk_types as types;

// Re-exports.
pub use self::{
    context::Context,
    contract::{Contract, Error},
};

// Use `wee_alloc` as the global allocator.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
