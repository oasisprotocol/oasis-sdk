//! Oasis WASM ABI implementation.
use crate::memory;

pub mod crypto;
pub mod dispatch;
pub mod env;
pub mod storage;
pub mod version;

#[no_mangle]
pub extern "C" fn allocate(length: u32) -> u32 {
    memory::allocate_host(length)
}

#[no_mangle]
pub extern "C" fn deallocate(offset: u32, length: u32) {
    memory::deallocate_host(offset, length)
}
