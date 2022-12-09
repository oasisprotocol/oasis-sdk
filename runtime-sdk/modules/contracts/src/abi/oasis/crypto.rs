//! Crypto function imports.
use std::convert::TryInto;

use oasis_contract_sdk_crypto as crypto;
use oasis_contract_sdk_types::crypto::SignatureKind;
use oasis_runtime_sdk::{context::Context, crypto::signature};

use super::{memory::Region, OasisV1};
use crate::{
    abi::{gas, ExecutionContext},
    Config, Error,
};

impl<Cfg: Config> OasisV1<Cfg> {
    /// Link crypto helper functions.
    pub fn link_crypto<C: Context>(
        instance: &mut wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
    ) -> Result<(), Error> {
        // crypto.ecdsa_recover(input) -> response
        let _ = instance.link_function(
            "crypto",
            "ecdsa_recover",
            |ctx, request: ((u32, u32), (u32, u32))| -> Result<(), wasm3::Trap> {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                // Charge gas.
                gas::use_gas(ctx.instance, ec.params.gas_costs.wasm_crypto_ecdsa_recover)?;

                let rt = ctx.instance.runtime();
                rt.try_with_memory(|mut memory| -> Result<_, wasm3::Trap> {
                    let input = Region::from_arg(request.0)
                        .as_slice(&memory)
                        .map_err(|_| wasm3::Trap::Abort)?
                        .to_vec();

                    let output: &mut [u8; 65] = Region::from_arg(request.1)
                        .as_slice_mut(&mut memory)
                        .map_err(|_| wasm3::Trap::Abort)?
                        .try_into()
                        .map_err(|_| wasm3::Trap::Abort)?;

                    match crypto::ecdsa::recover(&input) {
                        Ok(key) => output.copy_from_slice(&key),
                        Err(_) => output.iter_mut().for_each(|b| *b = 0),
                    }

                    Ok(())
                })?
            },
        );

        // crypto.signature_verify(public_key, context, message, signature) -> response
        #[allow(clippy::type_complexity)]
        let _ = instance.link_function(
            "crypto",
            "signature_verify",
            |ctx,
             (kind, key, context, message, signature): (
                u32,
                (u32, u32),
                (u32, u32),
                (u32, u32),
                (u32, u32),
            )|
             -> Result<u32, wasm3::Trap> {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                // Validate message length.
                if message.1 > ec.params.max_crypto_signature_verify_message_size_bytes {
                    ec.aborted = Some(Error::CryptoMsgTooLarge(
                        message.1,
                        ec.params.max_crypto_signature_verify_message_size_bytes,
                    ));
                    return Err(wasm3::Trap::Abort);
                }

                let kind: SignatureKind = kind.try_into().map_err(|_| wasm3::Trap::Abort)?;

                // Charge gas.
                let cost = match kind {
                    SignatureKind::Ed25519 => {
                        ec.params.gas_costs.wasm_crypto_signature_verify_ed25519
                    }
                    SignatureKind::Secp256k1 => {
                        ec.params.gas_costs.wasm_crypto_signature_verify_secp256k1
                    }
                    SignatureKind::Sr25519 => {
                        ec.params.gas_costs.wasm_crypto_signature_verify_sr25519
                    }
                };
                gas::use_gas(ctx.instance, cost)?;

                let rt = ctx.instance.runtime();
                rt.try_with_memory(|memory| -> Result<_, wasm3::Trap> {
                    let key = get_key(kind, key, &memory).map_err(|err| {
                        ec.aborted = Some(Error::CryptoMalformedPublicKey);
                        err
                    })?;
                    let message = Region::from_arg(message)
                        .as_slice(&memory)
                        .map_err(|_| wasm3::Trap::Abort)?;
                    let signature: signature::Signature = Region::from_arg(signature)
                        .as_slice(&memory)
                        .map_err(|_| wasm3::Trap::Abort)?
                        .to_vec()
                        .into();
                    if context.0 != 0 && context.1 != 0 && matches!(kind, SignatureKind::Sr25519) {
                        let context = Region::from_arg(context)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?;
                        Ok(1 - key.verify(context, message, &signature).is_ok() as u32)
                    } else {
                        Ok(1 - key.verify_raw(message, &signature).is_ok() as u32)
                    }
                })?
            },
        );

        // crypto.random_bytes(dst) -> bytes_written
        let _ = instance.link_function(
            "crypto",
            "random_bytes",
            |ctx,
             ((pers_ptr, pers_len, dst_ptr, dst_len),): ((u32, u32, u32, u32),)|
             -> Result<u32, wasm3::Trap> {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                let num_bytes = dst_len.min(1024 /* 1 KiB */);

                // Charge gas.
                let cost = ec
                    .params
                    .gas_costs
                    .wasm_crypto_random_bytes_byte
                    .checked_mul(num_bytes as u64 + pers_len as u64)
                    .and_then(|g| g.checked_add(ec.params.gas_costs.wasm_crypto_random_bytes_base))
                    .unwrap_or(u64::max_value()); // This will certainly exhaust the gas limit.
                gas::use_gas(ctx.instance, cost)?;

                let rt = ctx.instance.runtime();
                rt.try_with_memory(|mut memory| -> Result<_, wasm3::Trap> {
                    let pers = Region::from_arg((pers_ptr, pers_len))
                        .as_slice(&memory)
                        .map_err(|_| wasm3::Trap::Abort)?;
                    let mut rng = ec.tx_context.rng(pers).map_err(|e| {
                        ec.aborted = Some(e.into());
                        wasm3::Trap::Abort
                    })?;
                    let output = Region::from_arg((dst_ptr, num_bytes))
                        .as_slice_mut(&mut memory)
                        .map_err(|_| wasm3::Trap::Abort)?;
                    rand_core::RngCore::try_fill_bytes(&mut rng, output).map_err(|e| {
                        ec.aborted = Some(Error::ExecutionFailed(e.into()));
                        wasm3::Trap::Abort
                    })?;
                    Ok(num_bytes)
                })?
            },
        );

        // crypto.x25519_derive_symmetric(public_key, private_key) -> symmetric_key
        #[allow(clippy::type_complexity)]
        let _ = instance.link_function(
            "crypto",
            "x25519_derive_symmetric",
            |ctx,
             (public_key, private_key, output_key): ((u32, u32), (u32, u32), (u32, u32))|
             -> Result<u32, wasm3::Trap> {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                gas::use_gas(
                    ctx.instance,
                    ec.params.gas_costs.wasm_crypto_x25519_derive_symmetric,
                )?;

                ctx.instance
                    .runtime()
                    .try_with_memory(|mut memory| -> Result<_, wasm3::Trap> {
                        let public = Region::from_arg(public_key)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .to_vec();
                        let private = Region::from_arg(private_key)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .to_vec();
                        let output: &mut [u8; crypto::x25519::KEY_SIZE] =
                            Region::from_arg(output_key)
                                .as_slice_mut(&mut memory)
                                .map_err(|_| wasm3::Trap::Abort)?
                                .try_into()
                                .map_err(|_| wasm3::Trap::Abort)?;
                        let derived =
                            crypto::x25519::derive_symmetric(&public, &private).map_err(|e| {
                                let err = match e {
                                    crypto::x25519::Error::MalformedPublicKey => {
                                        Error::CryptoMalformedPublicKey
                                    }
                                    crypto::x25519::Error::MalformedPrivateKey => {
                                        Error::CryptoMalformedPrivateKey
                                    }
                                    crypto::x25519::Error::KeyDerivationFunctionFailure => {
                                        Error::CryptoKeyDerivationFunctionFailure
                                    }
                                };
                                ec.aborted = Some(err);
                                wasm3::Trap::Abort
                            })?;
                        if output.len() != derived.len() {
                            return Err(wasm3::Trap::Abort);
                        }
                        output.copy_from_slice(&derived);
                        Ok(0)
                    })?
            },
        );

        #[allow(clippy::type_complexity)]
        let deoxysii_factory =
            |func: fn(&[u8], &[u8], &[u8], &[u8]) -> Result<Vec<u8>, crypto::deoxysii::Error>| {
                move |ctx: wasm3::CallContext<'_, ExecutionContext<'_, C>>,
                      (key, nonce, message, additional_data): (
                    (u32, u32),
                    (u32, u32),
                    (u32, u32),
                    (u32, u32),
                )|
                      -> Result<u32, wasm3::Trap> {
                    // Make sure function was called in valid context.
                    let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                    gas::use_gas(
                        ctx.instance,
                        ec.params.gas_costs.wasm_crypto_deoxysii_base
                            + ec.params.gas_costs.wasm_crypto_deoxysii_byte
                                * (message.1 as u64 + additional_data.1 as u64),
                    )?;

                    let output = ctx.instance.runtime().try_with_memory(
                        |memory| -> Result<Option<Vec<u8>>, wasm3::Trap> {
                            let key = Region::from_arg(key)
                                .as_slice(&memory)
                                .map_err(|_| wasm3::Trap::Abort)?;
                            let nonce = Region::from_arg(nonce)
                                .as_slice(&memory)
                                .map_err(|_| wasm3::Trap::Abort)?;
                            let message = Region::from_arg(message)
                                .as_slice(&memory)
                                .map_err(|_| wasm3::Trap::Abort)?;
                            let additional_data = Region::from_arg(additional_data)
                                .as_slice(&memory)
                                .map_err(|_| wasm3::Trap::Abort)?;
                            func(key, nonce, message, additional_data)
                                .map(Some)
                                .or_else(|e| {
                                    let err = match e {
                                        crypto::deoxysii::Error::MalformedKey => {
                                            Error::CryptoMalformedKey
                                        }
                                        crypto::deoxysii::Error::MalformedNonce => {
                                            Error::CryptoMalformedNonce
                                        }
                                        crypto::deoxysii::Error::DecryptionFailed => {
                                            return Ok(None);
                                        }
                                    };
                                    ec.aborted = Some(err);
                                    Err(wasm3::Trap::Abort)
                                })
                        },
                    )??;

                    if let Some(output) = output {
                        let output_region = Self::allocate_and_copy(ctx.instance, &output)?;
                        Self::allocate_region(ctx.instance, output_region).map_err(|e| e.into())
                    } else {
                        Ok(0)
                    }
                }
            };

        // crypto.deoxysii_seal(key, nonce, plaintext_message, additional_data) -> encrypted_message
        let _ = instance.link_function(
            "crypto",
            "deoxysii_seal",
            deoxysii_factory(crypto::deoxysii::seal),
        );

        // crypto.deoxysii_open(key, nonce, encrypted_message, additional_data) -> plaintext_message
        let _ = instance.link_function(
            "crypto",
            "deoxysii_open",
            deoxysii_factory(crypto::deoxysii::open),
        );

        Ok(())
    }
}

fn get_key(
    kind: SignatureKind,
    key: (u32, u32),
    memory: &wasm3::Memory<'_>,
) -> Result<signature::PublicKey, wasm3::Trap> {
    let region = Region::from_arg(key)
        .as_slice(memory)
        .map_err(|_| wasm3::Trap::Abort)?;

    match kind {
        SignatureKind::Ed25519 => {
            let ed25519 = signature::ed25519::PublicKey::from_bytes(region)
                .map_err(|_| wasm3::Trap::Abort)?;
            Ok(signature::PublicKey::Ed25519(ed25519))
        }
        SignatureKind::Secp256k1 => {
            let secp256k1 = signature::secp256k1::PublicKey::from_bytes(region)
                .map_err(|_| wasm3::Trap::Abort)?;
            Ok(signature::PublicKey::Secp256k1(secp256k1))
        }
        SignatureKind::Sr25519 => {
            let sr25519 = signature::sr25519::PublicKey::from_bytes(region)
                .map_err(|_| wasm3::Trap::Abort)?;
            Ok(signature::PublicKey::Sr25519(sr25519))
        }
    }
}

#[cfg(all(feature = "benchmarks", test))]
mod test {
    extern crate test;
    use super::*;
    use test::Bencher;

    use k256::{
        self,
        ecdsa::{self, signature::Verifier as _},
    };

    // cargo build --target wasm32-unknown-unknown --release
    const BENCH_CODE: &[u8] = include_bytes!(
        "../../../../../../tests/contracts/bench/target/wasm32-unknown-unknown/release/bench.wasm"
    );
    const MESSAGE: &[u8] =
        include_bytes!("../../../../../../tests/contracts/bench/data/message.txt");
    const SIGNATURE: &[u8] =
        include_bytes!("../../../../../../tests/contracts/bench/data/signature.bin");
    const KEY: &[u8] = include_bytes!("../../../../../../tests/contracts/bench/data/key.bin");

    fn verify_signature(message: &[u8], signature: &[u8], key: &[u8]) -> Result<(), ()> {
        let key = k256::EncodedPoint::from_bytes(key).map_err(|_| ())?;
        let sig = ecdsa::Signature::from_der(signature).map_err(|_| ())?;
        let verifying_key = ecdsa::VerifyingKey::from_encoded_point(&key).map_err(|_| ())?;
        verifying_key.verify(message, &sig).map_err(|_| ())?;
        Ok(())
    }

    #[bench]
    fn bench_crypto_nonwasm_verify(b: &mut Bencher) {
        b.iter(|| {
            verify_signature(MESSAGE, SIGNATURE, KEY).unwrap();
        });
    }

    #[bench]
    fn bench_crypto_nonwasm_recover(b: &mut Bencher) {
        let input = hex::decode("ce0677bb30baa8cf067c88db9811f4333d131bf8bcf12fe7065d211dce97100890f27b8b488db00b00606796d2987f6a5f59ae62ea05effe84fef5b8b0e549984a691139ad57a3f0b906637673aa2f63d1f55cb1a69199d4009eea23ceaddc9301").unwrap();
        b.iter(|| {
            assert!(crypto::ecdsa::recover(&input).is_ok());
        });
    }

    #[bench]
    fn bench_crypto_nonwasm_x25519_derive(b: &mut Bencher) {
        let public = <[u8; 32] as hex::FromHex>::from_hex(
            "3046db3fa70ce605457dc47c48837ebd8bd0a26abfde5994d033e1ced68e2576",
        )
        .unwrap();
        let private = <[u8; 32] as hex::FromHex>::from_hex(
            "c07b151fbc1e7a11dff926111188f8d872f62eba0396da97c0a24adb75161750",
        )
        .unwrap();
        b.iter(|| {
            assert!(crypto::x25519::derive_symmetric(&public, &private).is_ok());
        });
    }

    #[bench]
    fn bench_crypto_nonwasm_deoxysii_seal_tiny(b: &mut Bencher) {
        let key = <[u8; 32] as hex::FromHex>::from_hex(
            "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586",
        )
        .unwrap();
        b.iter(|| {
            assert!(crypto::deoxysii::seal(&key, b"0123456789abcde", b"b", b"").is_ok());
        });
    }

    #[bench]
    fn bench_crypto_nonwasm_deoxysii_seal_size1(b: &mut Bencher) {
        let key = <[u8; 32] as hex::FromHex>::from_hex(
            "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586",
        )
        .unwrap();
        b.iter(|| {
            assert!(crypto::deoxysii::seal(&key, b"0123456789abcde", MESSAGE, b"").is_ok());
        });
    }

    #[bench]
    fn bench_crypto_nonwasm_deoxysii_seal_size2(b: &mut Bencher) {
        let key = <[u8; 32] as hex::FromHex>::from_hex(
            "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586",
        )
        .unwrap();
        let mut message = MESSAGE.to_vec();
        message.extend_from_slice(MESSAGE);
        b.iter(|| {
            assert!(crypto::deoxysii::seal(&key, b"0123456789abcde", &message, b"").is_ok());
        });
    }

    #[bench]
    fn bench_crypto_called_from_wasm_included(b: &mut Bencher) {
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(BENCH_CODE)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, wasm3::CallContext<'_, ()>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let mut instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let _ = instance.link_function(
            "bench",
            "verify_signature",
            |ctx,
             (message, signature, key): ((u32, u32), (u32, u32), (u32, u32))|
             -> Result<(), wasm3::Trap> {
                ctx.instance
                    .runtime()
                    .try_with_memory(|memory| -> Result<_, wasm3::Trap> {
                        let message = Region::from_arg(message)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?;
                        let signature = Region::from_arg(signature)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?;
                        let key = Region::from_arg(key)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?;
                        verify_signature(message, signature, key)
                            .map_err(|_| wasm3::Trap::Abort)?;
                        Ok(())
                    })?
            },
        );
        let func = instance
            .find_function::<(), ()>("call_verification_included")
            .expect("finding the entrypoint function should succeed");
        b.iter(|| {
            func.call(()).expect("function call should succeed");
        });
    }

    #[bench]
    fn bench_crypto_computed_in_wasm(b: &mut Bencher) {
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(BENCH_CODE)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, wasm3::CallContext<'_, ()>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let func = instance
            .find_function::<(), ()>("call_verification_internal")
            .expect("finding the entrypoint function should succeed");
        b.iter(|| {
            func.call(()).expect("function call should succeed");
        });
    }

    #[bench]
    fn bench_crypto_computed_in_wasm_instrumented(_b: &mut Bencher) {
        let mut module = walrus::ModuleConfig::new()
            .generate_producers_section(false)
            .parse(&BENCH_CODE)
            .unwrap();
        gas::transform(&mut module);
        let new_code = module.emit_wasm();

        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(&new_code)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, wasm3::CallContext<'_, ()>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let initial_gas = 1_000_000_000_000u64;
        instance
            .set_global(gas::EXPORT_GAS_LIMIT, initial_gas)
            .expect("setting gas limit should succeed");
        let func = instance
            .find_function::<(), ()>("call_verification_internal")
            .expect("finding the entrypoint function should succeed");
        func.call(()).expect("function call should succeed");

        let gas_limit: u64 = instance
            .get_global(gas::EXPORT_GAS_LIMIT)
            .expect("getting gas limit global should succeed");
        let gas_limit_exhausted: u64 = instance
            .get_global(gas::EXPORT_GAS_LIMIT_EXHAUSTED)
            .expect("getting gas limit exhausted global should succeed");
        println!(
            "  signature verification done, gas remaining {} [used: {}, exhausted flag: {}]",
            gas_limit,
            initial_gas - gas_limit,
            gas_limit_exhausted
        );
    }
}
