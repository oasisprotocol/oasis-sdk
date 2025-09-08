use std::sync::Arc;

use anyhow::{anyhow, Result};
use k256::{elliptic_curve::sec1::ToEncodedPoint, sha2::Digest as Sha2Digest};

use oasis_runtime_sdk::{
    core::common::crypto::hash::Hash,
    crypto::signature::Signer,
    types::{
        transaction::{self, AuthProof, UnverifiedTransaction},
    },
};

/// Sign and encode a transaction as an Ethereum RLP-encoded transaction.
/// 
/// This function was moved from rofl-app-core to rofl-appd to centralize
/// Ethereum transaction handling in the daemon layer.
pub fn sign_and_encode_as_ethereum_tx(
    tx: &transaction::Transaction,
    signers: &[Arc<dyn Signer>],
) -> Result<(Vec<u8>, Hash)> {
    // Ensure a single signer.
    if signers.len() != 1 {
        return Err(anyhow!(
            "ethereum transactions support only a single signer"
        ));
    }

    // Ensure we have a secp256k1 signer for Ethereum.
    let signer = &signers[0];
    if signer.public_key().key_type() != "secp256k1" {
        return Err(anyhow!("ethereum transactions require secp256k1 signer"));
    }

    // Extract transaction parameters based on method using existing EVM types.
    let (action, value, data) = match tx.call.method.as_str() {
        "evm.Call" => {
            let call: oasis_runtime_sdk_evm::types::Call =
                cbor::from_value(tx.call.body.clone())
                    .map_err(|e| anyhow!("failed to decode evm.Call body: {}", e))?;
            ("call", call.value, call.data)
        }
        "evm.Create" => {
            let create: oasis_runtime_sdk_evm::types::Create =
                cbor::from_value(tx.call.body.clone())
                    .map_err(|e| anyhow!("failed to decode evm.Create body: {}", e))?;
            ("create", create.value, create.init_code)
        }
        _ => {
            return Err(anyhow!("not an EVM transaction"));
        }
    };

    // Transaction parameters.
    let nonce = tx
        .auth_info
        .signer_info
        .first()
        .ok_or_else(|| anyhow!("no signer info"))?
        .nonce;
    let gas_price = tx.auth_info.fee.gas_price();
    let gas_limit = tx.auth_info.fee.gas;

    // Create Ethereum transaction action.
    let eth_action = match action {
        "call" => {
            let call: oasis_runtime_sdk_evm::types::Call = cbor::from_value(tx.call.body.clone())?;
            let address: primitive_types::H160 = call.address.0.into();
            ethereum::TransactionAction::Call(address)
        }
        "create" => ethereum::TransactionAction::Create,
        _ => return Err(anyhow!("invalid action type")),
    };

    // Create EIP-2930 Ethereum transaction.
    let eth_tx = ethereum::EIP2930Transaction {
        chain_id: 123, // TODO: Get actual chain id (or create a Legacy transaction without a Chain ID).
        nonce: primitive_types::U256::from(nonce),
        gas_price: primitive_types::U256::from(gas_price),
        gas_limit: primitive_types::U256::from(gas_limit),
        action: eth_action,
        value: primitive_types::U256(value.0),
        input: data,
        access_list: vec![],
        signature: ethereum::eip2930::TransactionSignature::new(
            false,
            primitive_types::H256::from_low_u64_be(1),
            primitive_types::H256::from_low_u64_be(1),
        )
        .unwrap(),
    };

    // Sign the transaction.
    let signed_tx = sign_ethereum_transaction(eth_tx, signer.as_ref())?;
    let unverified_tx = UnverifiedTransaction(
        signed_tx,
        vec![AuthProof::Module("evm.ethereum.v0".to_string())],
    );
    let raw_tx = cbor::to_vec(unverified_tx);
    let tx_hash = Hash::digest_bytes(&raw_tx);

    Ok((raw_tx, tx_hash))
}

fn sign_ethereum_transaction(
    mut tx: ethereum::EIP2930Transaction,
    signer: &dyn Signer,
) -> Result<Vec<u8>> {
    let message = tx.clone().to_message();
    let hash: [u8; 32] = message.hash().as_bytes().try_into().unwrap();

    let sig = signer
        .sign_raw(&hash)
        .map_err(|e| anyhow!("failed to sign Ethereum transaction: {}", e))?;

    let sig = k256::ecdsa::Signature::from_der(sig.as_ref())
        .map_err(|e| anyhow!("failed parsing ecdsa signature: {e}"))?;

    // Normalize to low-S.
    let sig = match sig.normalize_s() {
        Some(normalized) => normalized,
        None => sig, // Already low-S.
    };

    // Determine recovery id (0/1).
    let pk = signer.public_key();
    let vk = k256::ecdsa::VerifyingKey::from_sec1_bytes(pk.as_bytes())
        .map_err(|e| anyhow!("invalid public key: {}", e))?;
    let recid_u8 = [0u8, 1u8]
        .iter()
        .find_map(|&rid| {
            let recid = k256::ecdsa::recoverable::Id::new(rid).ok()?;
            let rsig = k256::ecdsa::recoverable::Signature::new(&sig, recid).ok()?;
            let recovered = rsig
                .recover_verifying_key_from_digest(k256::sha2::Sha256::new().chain_update(&hash))
                .ok()?;

            if recovered.to_encoded_point(false) == vk.to_encoded_point(false) {
                Some(rid)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("could not determine recovery id"))?;

    // Fill r,s,y.
    let mut r_be = [0u8; 32];
    let mut s_be = [0u8; 32];
    r_be.copy_from_slice(sig.r().to_bytes().as_slice());
    s_be.copy_from_slice(sig.s().to_bytes().as_slice());
    tx.signature = ethereum::eip2930::TransactionSignature::new(
        recid_u8 == 1,
        primitive_types::H256::from_slice(&r_be),
        primitive_types::H256::from_slice(&s_be),
    )
    .unwrap();

    let mut result = vec![0x01];
    result.extend_from_slice(&rlp::encode(&tx));
    Ok(result)
}