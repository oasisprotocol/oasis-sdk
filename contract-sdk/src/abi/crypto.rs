//! Crypto helpers ABI.

#[link(wasm_import_module = "crypto")]
extern "C" {
    #[link_name = "ecdsa_recover"]
    pub(crate) fn crypto_ecdsa_recover(
        input_ptr: u32,
        input_len: u32,
        output_ptr: u32,
        output_len: u32,
    );
}
