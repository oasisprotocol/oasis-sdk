use k256::{
    self,
    ecdsa::{self, signature::Verifier as _},
};

use core::convert::TryInto;

const MESSAGE: &[u8] = include_bytes!("../data/message.txt");
const SIGNATURE: &[u8] = include_bytes!("../data/signature.bin");
const KEY: &[u8] = include_bytes!("../data/key.bin");

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

fn verify_signature() -> Result<(), ()> {
    let key = k256::EncodedPoint::from_bytes(KEY).map_err(|_| ())?;
    let sig = ecdsa::Signature::from_der(SIGNATURE).map_err(|_| ())?;
    let verifying_key = ecdsa::VerifyingKey::from_encoded_point(&key).map_err(|_| ())?;
    verifying_key.verify(MESSAGE, &sig).map_err(|_| ())?;
    Ok(())
}

#[link(wasm_import_module = "bench")]
extern "C" {
    #[link_name = "verify_signature"]
    fn bench_verify_signature(
        message_ptr: u32,
        message_len: u32,
        signature_ptr: u32,
        signature_len: u32,
        key_ptr: u32,
        key_len: u32,
    );

    #[link_name = "plain_get"]
    fn bench_storage_plain_get(key_ptr: u32, key_length: u32) -> u32;
    #[link_name = "plain_insert"]
    fn bench_storage_plain_insert(
        key_ptr: u32,
        key_length: u32,
        value_ptr: u32,
        value_length: u32,
    ) -> u32;
    #[link_name = "plain_remove"]
    fn bench_storage_plain_remove(key_ptr: u32, key_length: u32) -> u32;
}

#[no_mangle]
pub extern "C" fn call_verification_included() {
    unsafe {
        bench_verify_signature(
            MESSAGE.as_ptr() as u32,
            MESSAGE.len() as u32,
            SIGNATURE.as_ptr() as u32,
            SIGNATURE.len() as u32,
            KEY.as_ptr() as u32,
            KEY.len() as u32,
        )
    }
}

#[no_mangle]
pub extern "C" fn call_verification_internal() {
    verify_signature().unwrap();
}

#[no_mangle]
pub extern "C" fn alloc(length: u32) -> u32 {
    let data: Vec<u8> = Vec::with_capacity(length as usize);
    let data_ptr = data.as_ptr() as usize;
    std::mem::forget(data);
    data_ptr as u32
}

#[no_mangle]
pub extern "C" fn bench_storage_get() {
    for i in 0..5_000 {
        let key = format!("key{}", i);
        let exp_value = format!("value{}", i);

        let blob_ptr = unsafe { bench_storage_plain_get(key.as_ptr() as u32, key.len() as u32) };
        let len_bytes: [u8; std::mem::size_of::<u32>()] = unsafe {
            std::slice::from_raw_parts(blob_ptr as *const u8, std::mem::size_of::<u32>())
        }
        .try_into()
        .unwrap();
        let value_len = u32::from_le_bytes(len_bytes);
        let value_slice = unsafe {
            std::slice::from_raw_parts(
                (blob_ptr + std::mem::size_of::<u32>() as u32) as *const u8,
                value_len as usize,
            )
        };

        assert_eq!(exp_value.as_bytes(), value_slice);
    }
}

#[no_mangle]
pub extern "C" fn bench_storage_insert(base: u32) -> u32 {
    let lim = 5_000;
    for i in 0..lim {
        let key = format!("key{}", base + i);
        let value = format!("value{}", base + i);

        unsafe {
            bench_storage_plain_insert(
                key.as_ptr() as u32,
                key.len() as u32,
                value.as_ptr() as u32,
                value.len() as u32,
            )
        };
    }
    lim
}

#[no_mangle]
pub extern "C" fn bench_storage_remove(base: u32) -> u32 {
    let lim = 5_000;
    for i in 0..lim {
        let key = format!("key{}", base + i);

        unsafe { bench_storage_plain_remove(key.as_ptr() as u32, key.len() as u32) };
    }
    lim
}

#[no_mangle]
pub extern "C" fn bench_storage_gas_consumer(max_length: u32) -> u32 {
    let mut value: Vec<u8> = Vec::with_capacity(max_length as usize);
    for i in 0..max_length {
        value.push((i % 25) as u8 + b'a');
    }
    let mut i = 0;
    loop {
        let key = format!("key{}", i);
        i += 1;
        unsafe {
            bench_storage_plain_insert(
                key.as_ptr() as u32,
                key.len() as u32,
                value.as_ptr() as u32,
                value.len() as u32,
            )
        };
    }
}

fn f(n: u64) -> u64 {
    if n < 3 {
        1
    } else {
        f(n - 1) + f(n - 2)
    }
}

#[no_mangle]
pub extern "C" fn waste_time(n: u64) -> u64 {
    f(n)
}
