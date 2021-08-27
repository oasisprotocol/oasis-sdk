//! Oasis WASM ABI implementation.
use crate::memory;

pub mod dispatch;
pub mod env;
pub mod storage;

#[no_mangle]
pub extern "wasm" fn allocate(length: u32) -> u32 {
    memory::allocate_host(length)
}

#[no_mangle]
pub extern "wasm" fn deallocate(offset: u32, length: u32) {
    memory::deallocate_host(offset, length)
}
