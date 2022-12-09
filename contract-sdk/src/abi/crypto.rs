//! Crypto helpers ABI.

#[link(wasm_import_module = "crypto")]
extern "C" {
    #[link_name = "ecdsa_recover"]
    pub(crate) fn ecdsa_recover(input_ptr: u32, input_len: u32, output_ptr: u32, output_len: u32);

    #[link_name = "signature_verify"]
    pub(crate) fn signature_verify(
        kind: u32,
        key_ptr: u32,
        key_len: u32,
        context_ptr: u32,
        context_len: u32,
        message_ptr: u32,
        message_len: u32,
        signature_ptr: u32,
        signature_len: u32,
    ) -> u32;

    #[link_name = "x25519_derive_symmetric"]
    pub(crate) fn x25519_derive_symmetric(
        public_key_ptr: u32,
        public_key_len: u32,
        private_key_ptr: u32,
        private_key_len: u32,
        output_key_ptr: u32,
        output_key_len: u32,
    ) -> u32;

    #[link_name = "deoxysii_seal"]
    pub(crate) fn deoxysii_seal(
        key_ptr: u32,
        key_len: u32,
        nonce_ptr: u32,
        nonce_len: u32,
        message_ptr: u32,
        message_len: u32,
        additional_data_ptr: u32,
        additional_data_len: u32,
    ) -> u32;

    #[link_name = "deoxysii_open"]
    pub(crate) fn deoxysii_open(
        key_ptr: u32,
        key_len: u32,
        nonce_ptr: u32,
        nonce_len: u32,
        message_ptr: u32,
        message_len: u32,
        additional_data_ptr: u32,
        additional_data_len: u32,
    ) -> u32;

    #[link_name = "random_bytes"]
    pub(crate) fn random_bytes(
        pers_ptr: u32,
        pers_len: u32,
        output_ptr: u32,
        output_len: u32,
    ) -> u32;
}
