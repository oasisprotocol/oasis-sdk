use std::{collections::BTreeSet, sync::Arc};

use rocket::{http::Status, serde::json::Json, State};
use serde_with::serde_as;

use oasis_runtime_sdk::types::transaction;
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
        value: TransactionValue,
        #[serde_as(as = "serde_with::hex::Hex")]
        data: Vec<u8>,
    },
}

/// Value representation that accepts either a string (decimal or 0x hex) or a JSON number.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum TransactionValue {
    String(String),
    Number(u128),
}

impl TransactionValue {
    fn into_u256(self) -> Result<evm::types::U256, String> {
        match self {
            TransactionValue::Number(value) => Ok(value.into()),
            TransactionValue::String(value) => parse_u256_string(value),
        }
    }
}

fn parse_u256_string(value: String) -> Result<evm::types::U256, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("transaction value string must not be empty".to_string());
    }
    let (radix, digits) = match trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(rest) => (16, rest),
        None => (10, trimmed),
    };
    if digits.is_empty() {
        return Err("transaction value string must contain digits".to_string());
    }
    evm::types::U256::from_str_radix(digits, radix)
        .map_err(|_| "transaction value string is not a valid unsigned integer".to_string())
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
    let mut tx = match body.into_inner().tx {
        Transaction::Std(data) => {
            cbor::from_slice(&data).map_err(|err| (Status::BadRequest, err.to_string()))?
        }
        Transaction::Eth {
            gas_limit,
            to,
            value,
            data,
        } => {
            let value = value.into_u256().map_err(|err| (Status::BadRequest, err))?;
            let (method, body) = if to.is_empty() {
                // Create.
                (
                    "evm.Create",
                    cbor::to_value(evm::types::Create {
                        value,
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
                        value,
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

    // Sign and submit transaction.
    let result = env
        .sign_and_submit_tx(signer, tx, opts)
        .await
        .map_err(|err| (Status::BadRequest, err.to_string()))?;

    // Encode the response.
    let response = SignAndSubmitResponse {
        data: cbor::to_vec(result),
    };

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::{evm, *};

    #[test]
    fn parse_u256_string_supports_decimal_and_hex() {
        let decimal = parse_u256_string("42".to_string()).unwrap();
        assert_eq!(decimal, evm::types::U256::from(42u32));

        let hex_lower = parse_u256_string("0x2a".to_string()).unwrap();
        assert_eq!(hex_lower, evm::types::U256::from(42u32));

        let hex_upper = parse_u256_string("  0X2A  ".to_string()).unwrap();
        assert_eq!(hex_upper, evm::types::U256::from(42u32));
    }

    #[test]
    fn parse_u256_string_rejects_invalid_inputs() {
        assert!(parse_u256_string("".to_string()).is_err());
        assert!(parse_u256_string("0x".to_string()).is_err());
        assert!(parse_u256_string("-1".to_string()).is_err());
        assert!(parse_u256_string("0xZZ".to_string()).is_err());
    }

    #[test]
    fn transaction_value_into_u256_handles_number_variant() {
        let value = TransactionValue::Number(10u128.pow(18));
        assert_eq!(
            value.into_u256().unwrap(),
            evm::types::U256::from(10u128.pow(18))
        );
    }

    #[test]
    fn transaction_value_into_u256_handles_string_variant() {
        let value = TransactionValue::String("1000".to_string());
        assert_eq!(value.into_u256().unwrap(), evm::types::U256::from(1000u32));
    }
}
