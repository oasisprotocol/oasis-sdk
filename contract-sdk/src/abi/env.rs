//! Environment query ABI.
use std::convert::TryFrom;

use oasis_contract_sdk_types::address::Address;

use crate::{
    abi::crypto,
    env::{Crypto, Env},
    memory::{HostRegion, HostRegionRef},
    types::{
        env::{QueryRequest, QueryResponse},
        InstanceId,
    },
};

#[link(wasm_import_module = "env")]
#[allow(unused)]
extern "C" {
    #[link_name = "query"]
    fn env_query(query_ptr: u32, query_len: u32) -> *const HostRegion;

    #[link_name = "address_for_instance"]
    fn env_address_for_instance(instance_id: u64, dst_ptr: u32, dst_len: u32);

    #[link_name = "debug_print"]
    fn env_debug_print(msg_ptr: u32, msg_len: u32);
}

/// Performs an environment query.
pub fn query(query: QueryRequest) -> QueryResponse {
    let query_data = cbor::to_vec(query);
    let query_region = HostRegionRef::from_slice(&query_data);
    let rsp_ptr = unsafe { env_query(query_region.offset, query_region.length) };
    let rsp_region = unsafe { HostRegion::deref(rsp_ptr) };

    // We expect the host to produce valid responses and abort otherwise.
    cbor::from_slice(&rsp_region.into_vec()).unwrap()
}

/// Host environment.
pub struct HostEnv;

impl Env for HostEnv {
    fn query<Q: Into<QueryRequest>>(&self, q: Q) -> QueryResponse {
        query(q.into())
    }

    fn address_for_instance(&self, instance_id: InstanceId) -> Address {
        // Prepare a region for response.
        let dst = [0; 21];
        let dst_region = HostRegionRef::from_slice(&dst);

        unsafe {
            env_address_for_instance(instance_id.as_u64(), dst_region.offset, dst_region.length)
        };

        // Parse the returned address.
        Address::try_from(dst.as_ref()).unwrap()
    }

    #[cfg(feature = "debug-utils")]
    fn debug_print(&self, msg: &str) {
        debug_print(msg)
    }
}

#[cfg(feature = "debug-utils")]
pub(crate) fn debug_print(msg: &str) {
    let msg_region = HostRegionRef::from_slice(msg.as_bytes());
    unsafe { env_debug_print(msg_region.offset, msg_region.length) };
}

impl Crypto for HostEnv {
    fn ecdsa_recover(&self, input: &[u8]) -> [u8; 65] {
        let input_region = HostRegionRef::from_slice(input);
        // Prepare a region for response.
        let dst = [0; 65];
        let dst_region = HostRegionRef::from_slice(&dst);

        unsafe {
            crypto::crypto_ecdsa_recover(
                input_region.offset,
                input_region.length,
                dst_region.offset,
                dst_region.length,
            )
        };

        dst
    }
}
