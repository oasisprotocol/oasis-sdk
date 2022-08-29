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

                ctx.instance
                    .runtime()
                    .try_with_memory(|mut memory| -> Result<_, wasm3::Trap> {
                        let input = Region::from_arg(request.0)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .to_vec();

                        let output: &mut [u8; 65] = Region::from_arg(request.1)
                            .as_slice_mut(&mut memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .try_into()
                            .map_err(|_| wasm3::Trap::Abort)?;

                        let key = crypto::ecdsa::recover(&input).unwrap_or([0; 65]);
                        output.copy_from_slice(&key);

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

                ctx.instance
                    .runtime()
                    .try_with_memory(|memory| -> Result<_, wasm3::Trap> {
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
                        if context.0 != 0
                            && context.1 != 0
                            && matches!(kind, SignatureKind::Sr25519)
                        {
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
