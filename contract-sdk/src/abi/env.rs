//! Environment query ABI.
use crate::{
    env::Env,
    memory::{HostRegion, HostRegionRef},
    types::env::{QueryRequest, QueryResponse},
};

#[link(wasm_import_module = "env")]
extern "wasm" {
    #[link_name = "query"]
    fn env_query(query_ptr: u32, query_len: u32) -> HostRegion;
}

/// Performs an environment query.
pub fn query(query: QueryRequest) -> QueryResponse {
    let query_data = cbor::to_vec(query);
    let query_region = HostRegionRef::from_slice(&query_data);
    let rsp_region = unsafe { env_query(query_region.offset, query_region.length) };

    // We expect the host to produce valid responses and abort otherwise.
    cbor::from_slice(&rsp_region.into_vec()).unwrap()
}

/// Host environment.
pub struct HostEnv;

impl Env for HostEnv {
    fn query<Q: Into<QueryRequest>>(&self, q: Q) -> QueryResponse {
        query(q.into())
    }
}
