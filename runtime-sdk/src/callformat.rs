//! Handling of different call formats.
use std::convert::TryInto;

use anyhow::anyhow;
use byteorder::{BigEndian, WriteBytesExt};
use oasis_core_runtime::consensus::beacon;
use rand_core::{OsRng, RngCore};

use crate::{
    context::Context,
    core::common::crypto::{mrae::deoxysii, x25519},
    crypto::signature::context::get_chain_context_for,
    keymanager, module,
    modules::core::Error,
    state::CurrentState,
    types::{
        self,
        transaction::{Call, CallFormat, CallResult},
    },
};

/// Maximum age of an ephemeral key in the number of epochs.
///
/// This is half the current window as enforced by the key manager as negative results are not
/// cached and randomized queries could open the scheme to a potential DoS attack.
const MAX_EPHEMERAL_KEY_AGE: beacon::EpochTime = 5;

/// Additional metadata required by the result encoding function.
pub enum Metadata {
    Empty,
    EncryptedX25519DeoxysII {
        /// Caller's ephemeral public key used for X25519.
        pk: x25519::PublicKey,
        /// Secret key.
        sk: x25519::PrivateKey,
        /// Transaction index within the batch.
        index: usize,
    },
}

impl std::fmt::Debug for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.debug_struct("Metadata::Empty").finish(),
            Self::EncryptedX25519DeoxysII { pk, index, .. } => f
                .debug_struct("Metadata::EncryptedX25519DeoxysII")
                .field("pk", pk)
                .field("index", index)
                .finish_non_exhaustive(),
        }
    }
}

/// Derive the key pair ID for the call data encryption key pair.
pub fn get_key_pair_id(epoch: beacon::EpochTime) -> keymanager::KeyPairId {
    keymanager::get_key_pair_id([
        get_chain_context_for(types::callformat::CALL_DATA_KEY_PAIR_ID_CONTEXT_BASE).as_slice(),
        &epoch.to_be_bytes(),
    ])
}

fn verify_epoch<C: Context>(ctx: &C, epoch: beacon::EpochTime) -> Result<(), Error> {
    if epoch > ctx.epoch() {
        return Err(Error::InvalidCallFormat(anyhow!("epoch in the future")));
    }
    if epoch < ctx.epoch().saturating_sub(MAX_EPHEMERAL_KEY_AGE) {
        return Err(Error::InvalidCallFormat(anyhow!(
            "epoch too far in the past"
        )));
    }
    Ok(())
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
            if !assume_km_reachable && CurrentState::with_env(|env| !env.is_execute()) {
                return Ok(None);
            }

            let decrypt = |epoch: beacon::EpochTime| {
                let keypair = key_manager
                    .get_or_create_ephemeral_keys(get_key_pair_id(epoch), epoch)
                    .map_err(|err| match err {
                        keymanager::KeyManagerError::InvalidEpoch(..) => {
                            Error::InvalidCallFormat(anyhow!("invalid epoch"))
                        }
                        _ => Error::Abort(err.into()),
                    })?;
                let sk = keypair.input_keypair.sk;
                // Derive shared secret via X25519 and open the sealed box.
                deoxysii::box_open(
                    &envelope.nonce,
                    envelope.data.clone(),
                    vec![],
                    &envelope.pk.0,
                    &sk.0,
                )
                .map(|data| (data, sk))
            };

            // Get transaction key pair from the key manager. Note that only the `input_keypair`
            // portion is used.
            let (data, sk) = if envelope.epoch > 0 {
                verify_epoch(ctx, envelope.epoch)?;
                decrypt(envelope.epoch)
            } else {
                // In case of failure, also try with previous epoch key in case the epoch
                // transition just occurred.
                decrypt(ctx.epoch()).or_else(|_| decrypt(ctx.epoch() - 1))
            }
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
    client_keypair: &(x25519_dalek::PublicKey, x25519_dalek::StaticSecret),
) -> Result<Call, Error> {
    match call.format {
        // In case of the plain-text data format, we simply pass on the call unchanged.
        CallFormat::Plain => Ok(call),

        // Encrypted data format using X25519 key exchange and Deoxys-II symmetric encryption.
        CallFormat::EncryptedX25519DeoxysII => {
            let key_manager = ctx.key_manager().ok_or_else(|| {
                Error::InvalidCallFormat(anyhow!("confidential transactions not available"))
            })?;
            let epoch = ctx.epoch();
            let runtime_keypair = key_manager
                .get_or_create_ephemeral_keys(get_key_pair_id(epoch), epoch)
                .map_err(|err| Error::Abort(err.into()))?;
            let runtime_pk = runtime_keypair.input_keypair.pk;
            let nonce = [0u8; deoxysii::NONCE_SIZE];

            Ok(Call {
                format: call.format,
                method: std::mem::take(&mut call.method),
                body: cbor::to_value(types::callformat::CallEnvelopeX25519DeoxysII {
                    pk: client_keypair.0.into(),
                    nonce,
                    epoch,
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
    encode_result_ex(ctx, result, metadata, false /* expose_failure */)
}

/// Encode call results.
///
/// If `expose_failure` is set, then this method will not encrypt errors.
pub fn encode_result_ex<C: Context>(
    ctx: &C,
    result: module::CallResult,
    metadata: Metadata,
    expose_failure: bool,
) -> CallResult {
    match metadata {
        // In case of the plain-text data format, we simply pass on the data unchanged.
        Metadata::Empty => result.into(),

        // Encrypted data format using X25519 key exchange and Deoxys-II symmetric encryption.
        Metadata::EncryptedX25519DeoxysII { pk, sk, index } => {
            // Serialize result.
            let result: CallResult = result.into();

            if expose_failure {
                if result.is_success() {
                    return CallResult::Ok(encrypt_result_x25519_deoxysii(
                        ctx, result, pk, sk, index,
                    ));
                }

                return result;
            }

            CallResult::Unknown(encrypt_result_x25519_deoxysii(ctx, result, pk, sk, index))
        }
    }
}

/// Encrypt a call result using the X25519-Deoxys-II encryption scheme.
pub fn encrypt_result_x25519_deoxysii<C: Context>(
    ctx: &C,
    result: types::transaction::CallResult,
    pk: x25519::PublicKey,
    sk: x25519::PrivateKey,
    index: usize,
) -> cbor::Value {
    let mut nonce = Vec::with_capacity(deoxysii::NONCE_SIZE);
    if CurrentState::with_env(|env| env.is_execute()) {
        // In execution mode generate nonce for the output as Round (8 bytes) || Index (4 bytes) || 00 00 00.
        nonce
            .write_u64::<BigEndian>(ctx.runtime_header().round)
            .unwrap();
        nonce
            .write_u32::<BigEndian>(index.try_into().unwrap())
            .unwrap();
        nonce.extend(&[0, 0, 0]);
    } else {
        // In non-execution mode randomize the nonce to facilitate private queries.
        nonce.resize(deoxysii::NONCE_SIZE, 0);
        OsRng.fill_bytes(&mut nonce);
    }
    let nonce = nonce.try_into().unwrap();
    let result = cbor::to_vec(result);
    let data = deoxysii::box_seal(&nonce, result, vec![], &pk.0, &sk.0).unwrap();

    // Return an envelope.
    cbor::to_value(types::callformat::ResultEnvelopeX25519DeoxysII { nonce, data })
}

#[cfg(any(test, feature = "test"))]
pub fn decode_result<C: Context>(
    ctx: &C,
    format: CallFormat,
    result: CallResult,
    client_keypair: &(x25519_dalek::PublicKey, x25519_dalek::StaticSecret),
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
                .get_or_create_ephemeral_keys(get_key_pair_id(ctx.epoch()), ctx.epoch())
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
