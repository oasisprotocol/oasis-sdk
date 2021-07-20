//! WASM export implementations required by the ABI.
use crate::memory;

#[no_mangle]
pub extern "wasm" fn allocate(length: u32) -> u32 {
    memory::allocate_host(length)
}

#[no_mangle]
pub extern "wasm" fn deallocate(offset: u32, length: u32) {
    memory::deallocate_host(offset, length)
}

#[macro_export]
macro_rules! create_contract {
    ($name:ty) => {
        #[no_mangle]
        pub extern "wasm" fn instantiate(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> $crate::memory::HostRegion {
            $crate::contract::instantiate::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }

        #[no_mangle]
        pub extern "wasm" fn call(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> $crate::memory::HostRegion {
            $crate::contract::call::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }
    };
}
