//! Handling of different call formats.
use std::convert::TryInto;

use anyhow::anyhow;
use byteorder::{BigEndian, WriteBytesExt};

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
pub fn get_key_pair_id<C: Context>(ctx: &C) -> keymanager::KeyPairId {
    keymanager::get_key_pair_id(&[
        &get_chain_context_for(types::callformat::CALL_DATA_KEY_PAIR_ID_CONTEXT_BASE),
        &ctx.epoch().to_be_bytes(),
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
                    .map_err(|_| Error::InvalidCallFormat(anyhow!("bad envelope")))?;

            // Make sure a key manager is available in this runtime.
            let key_manager = ctx.key_manager().ok_or_else(|| {
                Error::InvalidCallFormat(anyhow!("confidential transactions not available"))
            })?;

            // If we are only doing checks, this is the most that we can do as in this case we may
            // be unable to access the key manager.
            if !assume_km_reachable && (ctx.is_check_only() || ctx.is_simulation()) {
                return Ok(None);
            }

            // Get transaction key pair from the key manager. Note that only the `input_keypair`
            // portion is used.
            let keypair = key_manager
                .get_or_create_keys(get_key_pair_id(ctx))
                .map_err(|err| Error::Abort(err.into()))?;
            let sk = keypair.input_keypair.sk;
            // Derive shared secret via X25519 and open the sealed box.
            let data =
                deoxysii::box_open(&envelope.nonce, envelope.data, vec![], &envelope.pk, &sk.0)
                    .map_err(Error::InvalidCallFormat)?;
            let call = cbor::from_slice(&data)
                .map_err(|_| Error::InvalidCallFormat(anyhow!("malformed call")))?;
            Ok(Some((
                call,
                Metadata::EncryptedX25519DeoxysII {
                    pk: envelope.pk,
                    sk,
                    index,
                },
            )))
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
