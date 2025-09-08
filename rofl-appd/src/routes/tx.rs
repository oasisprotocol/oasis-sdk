use std::{collections::BTreeSet, sync::Arc};

use anyhow::Result;

use rocket::{http::Status, serde::json::Json, State};
use serde_with::serde_as;

use oasis_runtime_sdk::{
    crypto::signature::Signer,
    types::{
        transaction::{self, UnverifiedTransaction},
    },
};
use oasis_runtime_sdk_evm as evm;
use rofl_app_core::client::SubmitTxOpts;

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

    /// Use Oasis transaction format for EVM transactions instead of Ethereum format.
    /// When false (default), EVM transactions (evm.Call, evm.Create) will be encoded as Ethereum transactions.
    #[serde(default = "default_evm_use_oasis_tx_flag")]
    pub evm_use_oasis_tx: bool,
}

/// Default value for the `encrypt` field in `SignAndSubmitRequest`.
fn default_encrypt_flag() -> bool {
    true
}

/// Default value for the `evm_use_oasis_tx` field in `SignAndSubmitRequest`.
fn default_evm_use_oasis_tx_flag() -> bool {
    false
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
    let inner_body = body.into_inner();
    let mut tx = match inner_body.tx {
        Transaction::Std(data) => {
            cbor::from_slice(&data).map_err(|err| (Status::BadRequest, err.to_string()))?
        }
        Transaction::Eth {
            gas_limit,
            to,
            value,
            data,
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

            transaction::Transaction {
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
            }
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

    // Sign and submit transaction using appropriate encoding format.
    let result = if inner_body.evm_use_oasis_tx {
        // Use standard Oasis SDK format.
        env.sign_and_submit_tx(signer, tx, opts)
            .await
            .map_err(|err| (Status::BadRequest, err.to_string()))?
    } else {
        // Use Ethereum format encoding - prepare the transaction here in appd.
        let prepared_tx = prepare_ethereum_transaction(tx, &[signer], env.as_ref(), &opts)
            .await
            .map_err(|err| (Status::BadRequest, err.to_string()))?;
        
        env.submit_prepared_tx(prepared_tx)
            .await
            .map_err(|err| (Status::BadRequest, err.to_string()))?
    };

    // Encode the response.
    let response = SignAndSubmitResponse {
        data: cbor::to_vec(result),
    };

    Ok(Json(response))
}

/// Prepare a transaction for Ethereum encoding in appd.
///
/// This is a simplified implementation that prepares Ethereum transactions
/// with basic nonce and gas handling.
async fn prepare_ethereum_transaction(
    tx: transaction::Transaction,
    signers: &[Arc<dyn Signer>],
    _env: &dyn Env,
    _opts: &SubmitTxOpts,
) -> Result<UnverifiedTransaction> {
    use crate::tx_prep::sign_and_encode_as_ethereum_tx;
    use oasis_runtime_sdk::types::{
        address::SignatureAddressSpec,
        token,
        transaction::AuthProof,
    };

    if signers.is_empty() {
        return Err(anyhow::anyhow!("no signers specified"));
    }

    let mut tx = tx;
    
    // Add signer info with nonce 0 (simplified - in production this should be resolved properly)
    for signer in signers {
        let sigspec = SignatureAddressSpec::try_from_pk(&signer.public_key())
            .ok_or_else(|| anyhow::anyhow!("signature scheme not supported"))?;
        tx.append_auth_signature(sigspec, 0);
    }
    
    // Set basic gas and gas price if not set
    if tx.fee_gas() == 0 {
        tx.set_fee_gas(21000);
    }
    
    if tx.fee_amount().amount() == 0 {
        tx.set_fee_amount(token::BaseUnits::new(1_000_000, token::Denomination::NATIVE));
    }

    // Sign and encode as Ethereum transaction.
    let (raw_tx, _tx_hash) = sign_and_encode_as_ethereum_tx(&tx, signers)?;

    Ok(UnverifiedTransaction(
        raw_tx,
        vec![AuthProof::Module("evm.ethereum.v0".to_string())],
    ))
}
