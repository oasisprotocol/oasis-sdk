use std::{collections::BTreeSet, sync::Arc};

use anyhow::{anyhow, Result};
use k256::{elliptic_curve::sec1::ToEncodedPoint, sha2::Digest};
use rand::{rngs::OsRng, Rng};
use rocket::{http::Status, serde::json::Json, State};
use serde_with::serde_as;

use ethereum;
use oasis_runtime_sdk::{
    core::common::crypto::mrae::deoxysii,
    crypto::signature::Signer,
    types::{
        address::{Address, SignatureAddressSpec},
        callformat, token,
        transaction::{self, AuthProof, UnverifiedTransaction},
    },
};
use oasis_runtime_sdk_evm as evm;
use primitive_types;
use rlp;
use rofl_app_core::client::{EncryptionMeta, PrepareClient, SubmitTxOpts};

use crate::state::Env;

/// Transaction endpoint configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Allowed method names.
    pub allowed_methods: BTreeSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // A default set of safe methods to be used from ROFL apps. Specifically this disallows
            // key derivation to avoid bypassing the built-in KMS.
            allowed_methods: BTreeSet::from_iter(
                [
                    "accounts.Transfer",
                    "consensus.Deposit",
                    "consensus.Withdraw",
                    "consensus.Delegate",
                    "consensus.Undelegate",
                    "evm.Call",
                    "evm.Create",
                    "rofl.Create",
                    "rofl.Update",
                    "rofl.Remove",
                ]
                .iter()
                .map(|m| m.to_string()),
            ),
        }
    }
}

/// A type that can represent both standard and Ethereum transactions.
#[serde_as]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Transaction {
    /// Standard Oasis SDK transaction.
    #[serde(rename = "std")]
    Std(#[serde_as(as = "serde_with::hex::Hex")] Vec<u8>),

    /// Ethereum transaction.
    #[serde(rename = "eth")]
    Eth {
        gas_limit: u64,
        #[serde_as(as = "serde_with::hex::Hex")]
        to: Vec<u8>,
        value: u128,
        #[serde_as(as = "serde_with::hex::Hex")]
        data: Vec<u8>,
        /// Whether to use Oasis SDK transaction format (true) or native Ethereum format (false).
        #[serde(default)]
        use_oasis_tx: bool,
    },
}

/// Transaction signing and submission request.
#[serde_as]
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SignAndSubmitRequest {
    /// Transaction.
    pub tx: Transaction,

    /// Whether the transaction calldata should be encrypted.
    #[serde(default = "default_encrypt_flag")]
    pub encrypt: bool,
}

/// Default value for the `encrypt` field in `SignAndSubmitRequest`.
fn default_encrypt_flag() -> bool {
    true
}

/// Transaction signing and submission response.
#[serde_as]
#[derive(Clone, Default, serde::Serialize)]
pub struct SignAndSubmitResponse {
    /// Raw response data.
    #[serde_as(as = "serde_with::hex::Hex")]
    pub data: Vec<u8>,
}

/// Sign and submit a transaction to the registration paratime. The signer of the transaction
/// will be a key that is authenticated to represent this ROFL app instance.
#[rocket::post("/sign-submit", data = "<body>")]
pub async fn sign_and_submit(
    body: Json<SignAndSubmitRequest>,
    env: &State<Arc<dyn Env>>,
    cfg: &State<Config>,
) -> Result<Json<SignAndSubmitResponse>, (Status, String)> {
    // Grab the default transaction signer.
    let signer = env.signer();

    let opts = SubmitTxOpts {
        encrypt: body.encrypt,
        ..Default::default()
    };

    // Deserialize the passed transaction, depending on its kind.
    let (mut tx, encode_as_evm_tx) = match body.into_inner().tx {
        Transaction::Std(data) => (
            cbor::from_slice(&data).map_err(|err| (Status::BadRequest, err.to_string()))?,
            false,
        ),
        Transaction::Eth {
            gas_limit,
            to,
            value,
            data,
            use_oasis_tx,
        } => {
            let (method, body) = if to.is_empty() {
                // Create.
                (
                    "evm.Create",
                    cbor::to_value(evm::types::Create {
                        value: value.into(),
                        init_code: data,
                    }),
                )
            } else {
                // Call.
                let address = to
                    .as_slice()
                    .try_into()
                    .map_err(|_| (Status::BadRequest, "malformed address".to_string()))?;

                (
                    "evm.Call",
                    cbor::to_value(evm::types::Call {
                        address,
                        value: value.into(),
                        data,
                    }),
                )
            };

            let tx = transaction::Transaction {
                version: transaction::LATEST_TRANSACTION_VERSION,
                call: transaction::Call {
                    format: transaction::CallFormat::Plain,
                    method: method.to_owned(),
                    body,
                    ..Default::default()
                },
                auth_info: transaction::AuthInfo {
                    fee: transaction::Fee {
                        gas: gas_limit,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            };
            (tx, !use_oasis_tx)
        }
    };

    // Check if the method is authorised before signing.
    if tx.call.format != transaction::CallFormat::Plain {
        // Prevent bypassing the authorization check by encrypting the method name.
        return Err((
            Status::BadRequest,
            "use the encrypt flag for encryption".to_string(),
        ));
    }
    if !cfg.allowed_methods.contains(&tx.call.method) {
        return Err((
            Status::BadRequest,
            "transaction method not allowed".to_string(),
        ));
    }

    // Make the ROFL module resolve the payer for all of our transactions.
    tx.set_fee_proxy("rofl", env.app_id().as_ref());

    // Sign and submit transaction using the appropriate preparer.
    let preparer = if encode_as_evm_tx {
        prepare_evm_impl
    } else {
        prepare_sdk_impl
    };

    let result = env
        .sign_and_submit_tx_with_preparer(signer, tx, opts, preparer)
        .await
        .map_err(|err| (Status::BadRequest, err.to_string()))?;

    // Encode the response.
    let response = SignAndSubmitResponse {
        data: cbor::to_vec(result),
    };

    Ok(Json(response))
}

/// SDK preparer function wrapper.
pub fn prepare_sdk_impl<'a>(
    client: &'a dyn PrepareClient,
    signers: &'a [Arc<dyn Signer>],
    tx: transaction::Transaction,
    encrypt: bool,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<(transaction::UnverifiedTransaction, Option<EncryptionMeta>)>,
            > + Send
            + 'a,
    >,
> {
    Box::pin(rofl_app_core::client::prepare_sdk_impl(
        client, signers, tx, encrypt,
    ))
}

/// Prepare an EVM transaction.
pub fn prepare_evm_impl<'a>(
    client: &'a dyn PrepareClient,
    signers: &'a [Arc<dyn Signer>],
    tx: transaction::Transaction,
    encrypt: bool,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<(transaction::UnverifiedTransaction, Option<EncryptionMeta>)>,
            > + Send
            + 'a,
    >,
> {
    Box::pin(prepare_evm_impl_inner(client, signers, tx, encrypt))
}

async fn prepare_evm_impl_inner(
    client: &dyn PrepareClient,
    signers: &[Arc<dyn Signer>],
    tx: transaction::Transaction,
    encrypt: bool,
) -> Result<(transaction::UnverifiedTransaction, Option<EncryptionMeta>)> {
    // Ensure a single, compatible signer is used.
    if signers.is_empty() {
        return Err(anyhow!("no signers specified"));
    }
    if signers.len() != 1 {
        return Err(anyhow!(
            "ethereum transactions support only a single signer"
        ));
    }
    let signer = &signers[0];
    if signer.public_key().key_type() != "secp256k1" {
        return Err(anyhow!("ethereum transactions require secp256k1 signer"));
    }

    // Resolve signer addresses.
    let sigspec = SignatureAddressSpec::try_from_pk(&signer.public_key())
        .ok_or(anyhow!("signature scheme not supported"))?;
    let address = (Address::from_sigspec(&sigspec), sigspec);

    // Resolve account nonce.
    let round = client.latest_round().await?;
    let nonce = client.account_nonce(round, address.0).await?;

    // Determine gas price. Currently we always use the native denomination.
    let gas_price = client
        .gas_price(round, &token::Denomination::NATIVE)
        .await?;

    // Determine gas limit.
    let mut gas_limit = tx.fee_gas();
    if gas_limit == 0 {
        gas_limit = rofl_app_core::client::estimate_gas(
            signer,
            address.0,
            tx.clone(),
            encrypt,
            round,
            client,
        )
        .await?;
    }

    // Extract EVM transaction parameters.
    let (tx_action, tx_value, mut tx_input) = match tx.call.method.as_str() {
        "evm.Call" => {
            let call: evm::types::Call = cbor::from_value(tx.call.body.clone())
                .map_err(|e| anyhow!("failed to decode evm.Call body: {}", e))?;
            (
                ethereum::TransactionAction::Call(call.address.0.into()),
                call.value,
                call.data,
            )
        }
        "evm.Create" => {
            let create: evm::types::Create = cbor::from_value(tx.call.body.clone())
                .map_err(|e| anyhow!("failed to decode evm.Create body: {}", e))?;
            (
                ethereum::TransactionAction::Create,
                create.value,
                create.init_code,
            )
        }
        _ => {
            return Err(anyhow!("not an EVM transaction"));
        }
    };

    let meta = if encrypt {
        // Obtain runtime's current ephemeral public key.
        let runtime_pk = client.call_data_public_key().await?;
        // Generate local key pair and nonce.
        let client_kp = deoxysii::generate_key_pair();
        let mut nonce = [0u8; deoxysii::NONCE_SIZE];
        OsRng.fill(&mut nonce);

        // Encrypt and encode call as evm transaction input.
        tx_input = cbor::to_vec(transaction::Call {
            format: transaction::CallFormat::EncryptedX25519DeoxysII,
            method: "".to_string(),
            body: cbor::to_value(callformat::CallEnvelopeX25519DeoxysII {
                pk: client_kp.0.into(),
                nonce,
                epoch: runtime_pk.epoch,
                data: deoxysii::box_seal(
                    &nonce,
                    cbor::to_vec(transaction::Call {
                        body: cbor::to_value(tx_input),
                        ..Default::default()
                    }),
                    vec![],
                    &runtime_pk.public_key.key.0,
                    &client_kp.1,
                )?,
            }),
            ..Default::default()
        });

        Some((runtime_pk, client_kp))
    } else {
        None
    };

    // Create EIP-2930 Ethereum transaction.
    let eth_tx = ethereum::EIP2930Transaction {
        chain_id: 123, // TODO: Get actual chain id (or create a Legacy transaction without a Chain ID).
        nonce: primitive_types::U256::from(nonce),
        gas_price: primitive_types::U256::from(gas_price),
        gas_limit: primitive_types::U256::from(gas_limit),
        action: tx_action,
        value: primitive_types::U256(tx_value.0),
        input: tx_input,
        access_list: vec![],
        signature: ethereum::eip2930::TransactionSignature::new(
            false,
            primitive_types::H256::from_low_u64_be(1),
            primitive_types::H256::from_low_u64_be(1),
        )
        .unwrap(),
    };

    let signed = sign_ethereum_transaction(eth_tx, signer)?;
    let unverified_tx = UnverifiedTransaction(
        signed,
        vec![AuthProof::Module("evm.ethereum.v0".to_string())],
    );

    Ok((unverified_tx, meta))
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

#[cfg(test)]
mod tests {
    use super::*;
    use oasis_runtime_sdk::{crypto::signature::secp256k1, types::token};
    use oasis_runtime_sdk_evm::raw_tx;

    #[test]
    fn test_sign_ethereum_transaction() {
        let test_seed = [1u8; 32];
        let signer = secp256k1::MemorySigner::new_from_seed(&test_seed).unwrap();

        let eth_tx = ethereum::EIP2930Transaction {
            chain_id: 123,
            nonce: primitive_types::U256::from(42),
            gas_price: primitive_types::U256::from(1000000000u64), // 1 gwei
            gas_limit: primitive_types::U256::from(21000),
            action: ethereum::TransactionAction::Call([0x42u8; 20].into()),
            value: primitive_types::U256::from(1000000000000000000u128), // 1 ETH in wei
            input: vec![0x01, 0x02, 0x03, 0x04],
            access_list: vec![],
            signature: ethereum::eip2930::TransactionSignature::new(
                false,
                primitive_types::H256::from_low_u64_be(1),
                primitive_types::H256::from_low_u64_be(1),
            )
            .unwrap(),
        };

        let signed_tx_bytes = sign_ethereum_transaction(eth_tx, &signer).unwrap();

        let min_gas_price = 1000000000u128; // 1 gwei
        let denom = &token::Denomination::NATIVE;
        let decoded_tx = raw_tx::decode(&signed_tx_bytes, Some(123), min_gas_price, denom);
        assert!(
            decoded_tx.is_ok(),
            "Failed to decode signed ethereum transaction: {:?}",
            decoded_tx.err()
        );

        let tx = decoded_tx.unwrap();
        assert_eq!(tx.fee_gas(), 21000, "Gas limit should match");
        assert!(
            tx.call.method == "evm.Call",
            "Should be an evm.Call transaction"
        );
    }
}
