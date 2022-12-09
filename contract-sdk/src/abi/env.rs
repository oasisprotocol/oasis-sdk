//! Environment query ABI.
use std::convert::TryFrom;

use oasis_contract_sdk_types::address::Address;

use crate::{
    abi::crypto,
    env::{Crypto, CryptoError, Env},
    memory::{HostRegion, HostRegionRef},
    types::{
        crypto::SignatureKind,
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

impl HostEnv {
    fn signature_verify(
        &self,
        kind: SignatureKind,
        key: &[u8],
        context: Option<&[u8]>,
        message: &[u8],
        signature: &[u8],
    ) -> bool {
        let key_region = HostRegionRef::from_slice(key);
        let (ctx_offset, ctx_length) = match context {
            Some(context) if matches!(kind, SignatureKind::Sr25519) => {
                let region = HostRegionRef::from_slice(context);
                (region.offset, region.length)
            }
            _ => (0, 0),
        };
        let message_region = HostRegionRef::from_slice(message);
        let signature_region = HostRegionRef::from_slice(signature);
        let result = unsafe {
            crypto::signature_verify(
                kind as u32,
                key_region.offset,
                key_region.length,
                ctx_offset,
                ctx_length,
                message_region.offset,
                message_region.length,
                signature_region.offset,
                signature_region.length,
            )
        };
        result == 0
    }

    fn deoxysii_process(
        &self,
        func: unsafe extern "C" fn(u32, u32, u32, u32, u32, u32, u32, u32) -> u32,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        additional_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let key_region = HostRegionRef::from_slice(key);
        let nonce_region = HostRegionRef::from_slice(nonce);
        let message_region = HostRegionRef::from_slice(message);
        let additional_data_region = HostRegionRef::from_slice(additional_data);

        unsafe {
            let output_region_ptr = func(
                key_region.offset,
                key_region.length,
                nonce_region.offset,
                nonce_region.length,
                message_region.offset,
                message_region.length,
                additional_data_region.offset,
                additional_data_region.length,
            );
            if output_region_ptr == 0 {
                Err(CryptoError::DecryptionFailed)
            } else {
                Ok(HostRegion::deref(output_region_ptr as *const HostRegion).into_vec())
            }
        }
    }
}

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
            crypto::ecdsa_recover(
                input_region.offset,
                input_region.length,
                dst_region.offset,
                dst_region.length,
            )
        };

        dst
    }

    fn signature_verify_ed25519(&self, key: &[u8], message: &[u8], signature: &[u8]) -> bool {
        HostEnv::signature_verify(self, SignatureKind::Ed25519, key, None, message, signature)
    }

    fn signature_verify_secp256k1(&self, key: &[u8], message: &[u8], signature: &[u8]) -> bool {
        HostEnv::signature_verify(
            self,
            SignatureKind::Secp256k1,
            key,
            None,
            message,
            signature,
        )
    }

    fn signature_verify_sr25519(
        &self,
        key: &[u8],
        context: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> bool {
        HostEnv::signature_verify(
            self,
            SignatureKind::Sr25519,
            key,
            Some(context),
            message,
            signature,
        )
    }

    fn x25519_derive_symmetric(&self, public_key: &[u8], private_key: &[u8]) -> [u8; 32] {
        let public_region = HostRegionRef::from_slice(public_key);
        let private_region = HostRegionRef::from_slice(private_key);

        let output = [0u8; 32];
        let output_region = HostRegionRef::from_slice(&output);

        unsafe {
            crypto::x25519_derive_symmetric(
                public_region.offset,
                public_region.length,
                private_region.offset,
                private_region.length,
                output_region.offset,
                output_region.length,
            )
        };

        output
    }

    fn deoxysii_seal(
        &self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        additional_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        self.deoxysii_process(crypto::deoxysii_seal, key, nonce, message, additional_data)
    }

    fn deoxysii_open(
        &self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        additional_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        self.deoxysii_process(crypto::deoxysii_open, key, nonce, message, additional_data)
    }

    fn random_bytes(&self, pers: &[u8], dst: &mut [u8]) -> usize {
        let pers_region = HostRegionRef::from_slice(pers);
        let dst_region = HostRegionRef::from_slice(dst);
        unsafe {
            crypto::random_bytes(
                pers_region.offset,
                pers_region.length,
                dst_region.offset,
                dst_region.length,
            ) as usize
        }
    }
}
