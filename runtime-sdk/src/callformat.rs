//! Handling of different call formats.
use std::convert::TryInto;

use anyhow::anyhow;
use byteorder::{BigEndian, WriteBytesExt};
use oasis_core_runtime::consensus::beacon;

use crate::{
    context::Context,
    core::common::crypto::mrae::deoxysii,
    crypto::signature::context::get_chain_context_for,
    keymanager, module,
    modules::core::Error,
    types::{
        self,
        transaction::{Call, CallFormat, CallResult},
    },
};

/// Additional metadata required by the result encoding function.
pub enum Metadata {
    Empty,
    EncryptedX25519DeoxysII {
        /// Caller's ephemeral public key used for X25519.
        pk: [u8; 32],
        /// Secret key.
        sk: keymanager::PrivateKey,
        /// Transaction index within the batch.
        index: usize,
    },
}

/// Derive the key pair ID for the call data encryption key pair.
pub fn get_key_pair_id(epoch: beacon::EpochTime) -> keymanager::KeyPairId {
    keymanager::get_key_pair_id(&[
        &get_chain_context_for(types::callformat::CALL_DATA_KEY_PAIR_ID_CONTEXT_BASE),
        &epoch.to_be_bytes(),
    ])
}

/// Decode call arguments.
///
/// Returns `Some((Call, Metadata))` when processing should proceed and `None` in case further
/// execution needs to be deferred (e.g., because key manager access is required).
pub fn decode_call<C: Context>(
    ctx: &C,
    call: Call,
    index: usize,
) -> Result<Option<(Call, Metadata)>, Error> {
    decode_call_ex(ctx, call, index, false /* assume_km_reachable */)
}

/// Decode call arguments.
///
/// Returns `Some((Call, Metadata))` when processing should proceed and `None` in case further
/// execution needs to be deferred (e.g., because key manager access is required).
/// If `assume_km_reachable` is set, then this method will return errors instead of `None`.
pub fn decode_call_ex<C: Context>(
    ctx: &C,
    call: Call,
    index: usize,
    assume_km_reachable: bool,
) -> Result<Option<(Call, Metadata)>, Error> {
    match call.format {
        // In case of the plain-text data format, we simply pass on the call unchanged.
        CallFormat::Plain => Ok(Some((call, Metadata::Empty))),

        // Encrypted data format using X25519 key exchange and Deoxys-II symmetric encryption.
        CallFormat::EncryptedX25519DeoxysII => {
            // Method must be empty.
            if !call.method.is_empty() {
                return Err(Error::InvalidCallFormat(anyhow!("non-empty method")));
            }
            // Body needs to follow the specified envelope.
            let envelope: types::callformat::CallEnvelopeX25519DeoxysII =
                cbor::from_value(call.body)
                    .map_err(|_| Error::InvalidCallFormat(anyhow!("bad call envelope")))?;
            let pk = envelope.pk;

            // Make sure a key manager is available in this runtime.
            let key_manager = ctx
                .key_manager()
                .ok_or_else(|| Error::InvalidCallFormat(anyhow!("confidential txs unavailable")))?;

            // If we are only doing checks, this is the most that we can do as in this case we may
            // be unable to access the key manager.
            if !assume_km_reachable && (ctx.is_check_only() || ctx.is_simulation()) {
                return Ok(None);
            }

            let decrypt = |epoch: beacon::EpochTime| {
                let keypair = key_manager
                    .get_or_create_keys(get_key_pair_id(epoch))
                    .map_err(|err| Error::Abort(err.into()))?;
                let sk = keypair.input_keypair.sk;
                // Derive shared secret via X25519 and open the sealed box.
                deoxysii::box_open(
                    &envelope.nonce,
                    envelope.data.clone(),
                    vec![],
                    &envelope.pk,
                    &sk.0,
                )
                .map(|data| (data, sk))
            };

            // Get transaction key pair from the key manager. Note that only the `input_keypair`
            // portion is used.  In case of failure, also try with previous epoch key in case the epoch
            // transition just occurred.
            let (data, sk) = decrypt(ctx.epoch())
                .or_else(|_| decrypt(ctx.epoch() - 1))
                .map_err(Error::InvalidCallFormat)?;

            let read_only = call.read_only;
            let call: Call = cbor::from_slice(&data)
                .map_err(|_| Error::InvalidCallFormat(anyhow!("malformed call")))?;

            // Ensure read-only flag is the same as in the outer envelope. This is to prevent
            // bypassing any authorization based on the read-only flag.
            if call.read_only != read_only {
                return Err(Error::InvalidCallFormat(anyhow!("read-only flag mismatch")));
            }

            Ok(Some((
                call,
                Metadata::EncryptedX25519DeoxysII { pk, sk, index },
            )))
        }
    }
}

#[cfg(any(test, feature = "test"))]
/// Encodes a call such that it can be decoded by `decode_call[_ex]`.
pub fn encode_call<C: Context>(
    ctx: &C,
    mut call: Call,
    client_keypair: &([u8; 32], [u8; 32]),
) -> Result<Call, Error> {
    match call.format {
        // In case of the plain-text data format, we simply pass on the call unchanged.
        CallFormat::Plain => Ok(call),

        // Encrypted data format using X25519 key exchange and Deoxys-II symmetric encryption.
        CallFormat::EncryptedX25519DeoxysII => {
            let key_manager = ctx.key_manager().ok_or_else(|| {
                Error::InvalidCallFormat(anyhow!("confidential transactions not available"))
            })?;
            let runtime_keypair = key_manager
                .get_or_create_keys(get_key_pair_id(ctx.epoch()))
                .map_err(|err| Error::Abort(err.into()))?;
            let runtime_pk = runtime_keypair.input_keypair.pk;
            let nonce = [0u8; deoxysii::NONCE_SIZE];

            Ok(Call {
                format: call.format,
                method: std::mem::take(&mut call.method),
                body: cbor::to_value(types::callformat::CallEnvelopeX25519DeoxysII {
                    pk: client_keypair.0,
                    nonce,
                    data: deoxysii::box_seal(
                        &nonce,
                        cbor::to_vec(call),
                        vec![],
                        &runtime_pk.0,
                        &client_keypair.1,
                    )
                    .unwrap(),
                }),
                ..Default::default()
            })
        }
    }
}

/// Encode call results.
pub fn encode_result<C: Context>(
    ctx: &C,
    result: module::CallResult,
    metadata: Metadata,
) -> CallResult {
    match metadata {
        // In case of the plain-text data format, we simply pass on the data unchanged.
        Metadata::Empty => result.into(),

        // Encrypted data format using X25519 key exchange and Deoxys-II symmetric encryption.
        Metadata::EncryptedX25519DeoxysII { pk, sk, index } => {
            // Generate nonce for the output as Round (8 bytes) || Index (4 bytes) || 00 00 00.
            let mut nonce = Vec::with_capacity(deoxysii::NONCE_SIZE);
            nonce
                .write_u64::<BigEndian>(ctx.runtime_header().round)
                .unwrap();
            nonce
                .write_u32::<BigEndian>(index.try_into().unwrap())
                .unwrap();
            nonce.extend(&[0, 0, 0]);
            let nonce = nonce.try_into().unwrap();
            // Serialize result.
            let result: CallResult = result.into();
            let result = cbor::to_vec(result);
            // Seal the result.
            let data = deoxysii::box_seal(&nonce, result, vec![], &pk, &sk.0).unwrap();

            // Generate an envelope.
            let envelope =
                cbor::to_value(types::callformat::ResultEnvelopeX25519DeoxysII { nonce, data });

            CallResult::Unknown(envelope)
        }
    }
}

#[cfg(any(test, feature = "test"))]
pub fn decode_result<C: Context>(
    ctx: &C,
    format: CallFormat,
    result: CallResult,
    client_keypair: &([u8; 32], [u8; 32]),
) -> Result<module::CallResult, Error> {
    if matches!(format, CallFormat::Plain) {
        return Ok(result.into_call_result().expect("CallResult was Unknown"));
    }
    let envelope_value = match result {
        CallResult::Ok(v) | CallResult::Unknown(v) => v,
        CallResult::Failed {
            module,
            code,
            message,
        } => {
            return Ok(module::CallResult::Failed {
                module,
                code,
                message,
            })
        }
    };
    match format {
        CallFormat::Plain => unreachable!("checked above"),
        CallFormat::EncryptedX25519DeoxysII => {
            let envelope: types::callformat::ResultEnvelopeX25519DeoxysII =
                cbor::from_value(envelope_value)
                    .map_err(|_| Error::InvalidCallFormat(anyhow!("bad result envelope")))?;

            // Get the runtime pubkey from the KM. A real client would simply use the
            // session key that has already been derived.
            let key_manager = ctx
                .key_manager()
                .ok_or_else(|| Error::InvalidCallFormat(anyhow!("confidential txs unavailable")))?;
            let keypair = key_manager
                .get_or_create_keys(get_key_pair_id(ctx.epoch()))
                .map_err(|err| Error::Abort(err.into()))?;
            let runtime_pk = keypair.input_keypair.pk;

            let data = deoxysii::box_open(
                &envelope.nonce,
                envelope.data,
                vec![],
                &runtime_pk.0,
                &client_keypair.1,
            )
            .map_err(Error::InvalidCallFormat)?;
            let call_result: CallResult = cbor::from_slice(&data)
                .map_err(|_| Error::InvalidCallFormat(anyhow!("malformed call")))?;
            Ok(call_result
                .into_call_result()
                .expect("CallResult was Unknown"))
        }
    }
}
