use std::{collections::BTreeSet, sync::Arc};

use rocket::{http::Status, serde::json::Json, State};
use serde::Deserialize;
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
        gas_limit: QuantityU64,
        #[serde(deserialize_with = "deserialize_hex_bytes")]
        to: Vec<u8>,
        value: TransactionValue,
        #[serde(deserialize_with = "deserialize_hex_bytes")]
        data: Vec<u8>,
    },
}

/// A quantity that can be provided either as a JSON number or as a string
/// (decimal or 0x-prefixed hex) and normalized into a `u64`.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum QuantityU64 {
    Number(u64),
    String(String),
}

impl QuantityU64 {
    fn into_u64(self, field: &'static str) -> Result<u64, String> {
        match self {
            QuantityU64::Number(n) => Ok(n),
            QuantityU64::String(s) => parse_u64_string(s, field),
        }
    }
}

/// Value representation that accepts either a string (decimal or 0x hex) or a JSON number.
///
/// Note: The `Number` variant uses `u64` because serde_json does not support `u128` in untagged
/// enums. For values larger than `u64::MAX`, use the `String` variant with a decimal or hex
/// representation.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum TransactionValue {
    String(String),
    Number(u64),
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

fn parse_u64_string(value: String, field: &'static str) -> Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} string must not be empty"));
    }
    let (radix, digits) = match trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(rest) => (16, rest),
        None => (10, trimmed),
    };
    if digits.is_empty() {
        return Err(format!("{field} string must contain digits"));
    }
    u64::from_str_radix(digits, radix)
        .map_err(|_| format!("{field} is not a valid unsigned 64-bit integer"))
}

/// Deserialize a hex string (with optional 0x prefix) into bytes.
fn deserialize_hex_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let trimmed = s.trim();
    let without_prefix = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    if without_prefix.is_empty() {
        return Ok(Vec::new());
    }

    hex::decode(without_prefix)
        .map_err(|e| serde::de::Error::custom(format!("invalid hex string: {e}")))
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
            let gas_limit = gas_limit
                .into_u64("gas_limit")
                .map_err(|err| (Status::BadRequest, err))?;
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
        let value = TransactionValue::Number(10u64.pow(18));
        assert_eq!(
            value.into_u256().unwrap(),
            evm::types::U256::from(10u64.pow(18))
        );
    }

    #[test]
    fn transaction_value_into_u256_handles_string_variant() {
        let value = TransactionValue::String("1000".to_string());
        assert_eq!(value.into_u256().unwrap(), evm::types::U256::from(1000u32));
    }

    #[test]
    fn transaction_value_json_deserializes_number() {
        // Test JSON number deserialization.
        let json = r#"0"#;
        let value: TransactionValue = serde_json::from_str(json).unwrap();
        assert_eq!(value.into_u256().unwrap(), evm::types::U256::from(0u32));

        let json = r#"1000000000000000000"#;
        let value: TransactionValue = serde_json::from_str(json).unwrap();
        assert_eq!(
            value.into_u256().unwrap(),
            evm::types::U256::from(10u64.pow(18))
        );
    }

    #[test]
    fn transaction_value_json_deserializes_string() {
        // Test JSON string deserialization (decimal).
        let json = r#""1000""#;
        let value: TransactionValue = serde_json::from_str(json).unwrap();
        assert_eq!(value.into_u256().unwrap(), evm::types::U256::from(1000u32));

        // Test JSON string deserialization (hex).
        let json = r#""0x3e8""#;
        let value: TransactionValue = serde_json::from_str(json).unwrap();
        assert_eq!(value.into_u256().unwrap(), evm::types::U256::from(1000u32));

        // Test large values via string (exceeds u64::MAX).
        let json = r#""340282366920938463463374607431768211455""#; // u128::MAX
        let value: TransactionValue = serde_json::from_str(json).unwrap();
        assert!(value.into_u256().is_ok());

        // Test 20B ROSE in wei (requires 95 bits): 20_000_000_000 * 10^18.
        let json = r#""20000000000000000000000000000""#;
        let value: TransactionValue = serde_json::from_str(json).unwrap();
        assert!(value.into_u256().is_ok());
    }

    #[test]
    fn parse_u64_string_supports_decimal_and_hex() {
        let decimal = parse_u64_string("21000".to_string(), "gas_limit").unwrap();
        assert_eq!(decimal, 21000u64);

        let hex_lower = parse_u64_string("0x2290b0".to_string(), "gas_limit").unwrap();
        assert_eq!(hex_lower, 0x2290b0u64);

        let hex_upper = parse_u64_string("  0X2290B0  ".to_string(), "gas_limit").unwrap();
        assert_eq!(hex_upper, 0x2290b0u64);
    }

    #[test]
    fn parse_u64_string_rejects_invalid_inputs() {
        assert!(parse_u64_string("".to_string(), "gas_limit").is_err());
        assert!(parse_u64_string("0x".to_string(), "gas_limit").is_err());
        assert!(parse_u64_string("-1".to_string(), "gas_limit").is_err());
        assert!(parse_u64_string("0xZZ".to_string(), "gas_limit").is_err());
    }

    #[test]
    fn eth_transaction_json_deserializes_with_value_variants() {
        // Test full Transaction deserialization with numeric value.
        let json = r#"{
            "kind": "eth",
            "data": {
                "gas_limit": "0x5208",
                "to": "0x0102030405060708091011121314151617181920",
                "value": 0,
                "data": "0x"
            }
        }"#;
        let tx: Transaction = serde_json::from_str(json).unwrap();
        match tx {
            Transaction::Eth { value, .. } => {
                assert_eq!(value.into_u256().unwrap(), evm::types::U256::from(0u32));
            }
            _ => panic!("Expected Eth transaction"),
        }

        // Test full Transaction deserialization with string value.
        let json = r#"{
            "kind": "eth",
            "data": {
                "gas_limit": 21000,
                "to": "0102030405060708091011121314151617181920",
                "value": "1000000000000000000",
                "data": ""
            }
        }"#;
        let tx: Transaction = serde_json::from_str(json).unwrap();
        match tx {
            Transaction::Eth { value, .. } => {
                assert_eq!(
                    value.into_u256().unwrap(),
                    evm::types::U256::from(10u64.pow(18))
                );
            }
            _ => panic!("Expected Eth transaction"),
        }
    }

    #[test]
    fn sign_and_submit_request_json_deserializes_with_encrypt_default() {
        // Test that `encrypt` defaults to true when omitted.
        let json = r#"{
            "tx": {
                "kind": "eth",
                "data": {
                    "gas_limit": "21000",
                    "to": "0x0102030405060708091011121314151617181920",
                    "value": "0",
                    "data": "0x"
                }
            }
        }"#;
        let req: SignAndSubmitRequest = serde_json::from_str(json).unwrap();
        assert!(req.encrypt, "encrypt should default to true");

        // Test explicit encrypt: false.
        let json = r#"{
            "tx": {
                "kind": "eth",
                "data": {
                    "gas_limit": 21000,
                    "to": "0102030405060708091011121314151617181920",
                    "value": "0",
                    "data": ""
                }
            },
            "encrypt": false
        }"#;
        let req: SignAndSubmitRequest = serde_json::from_str(json).unwrap();
        assert!(!req.encrypt);
    }

    #[test]
    fn sign_and_submit_request_json_deserializes_with_numeric_value() {
        // Test: Ensure SignAndSubmitRequest deserializes when value is a JSON number
        // (not just a string).
        let json = r#"{
            "tx": {
                "kind": "eth",
                "data": {
                    "gas_limit": 21000,
                    "to": "0x0102030405060708091011121314151617181920",
                    "value": 0,
                    "data": "0x"
                }
            }
        }"#;
        let req: SignAndSubmitRequest = serde_json::from_str(json).unwrap();
        match req.tx {
            Transaction::Eth { value, .. } => {
                assert_eq!(value.into_u256().unwrap(), evm::types::U256::from(0u32));
            }
            _ => panic!("Expected Eth transaction"),
        }

        // Also test with a realistic wei amount as JSON number (1 ETH = 10^18 wei).
        let json = r#"{
            "tx": {
                "kind": "eth",
                "data": {
                    "gas_limit": "0x30d40",
                    "to": "0102030405060708091011121314151617181920",
                    "value": 1000000000000000000,
                    "data": "deadbeef"
                }
            },
            "encrypt": true
        }"#;
        let req: SignAndSubmitRequest = serde_json::from_str(json).unwrap();
        assert!(req.encrypt);
        match req.tx {
            Transaction::Eth { value, .. } => {
                assert_eq!(
                    value.into_u256().unwrap(),
                    evm::types::U256::from(10u64.pow(18))
                );
            }
            _ => panic!("Expected Eth transaction"),
        }
    }
}
