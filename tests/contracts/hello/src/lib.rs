//! An example hello world contract also used in unit tests.
extern crate alloc;

use std::convert::TryInto;

use oas20::types::{ReceiverRequest, Request as Oas20Request, TokenInstantiation};

use oasis_contract_sdk::{
    self as sdk,
    env::{Crypto, Env},
    storage::Store,
    types::{
        crypto::SignatureKind,
        env::{AccountsQuery, AccountsResponse, QueryRequest, QueryResponse},
        message::{CallResult, Message, NotifyReply, Reply},
        modules::contracts::InstantiateResult,
        token, CodeId, InstanceId,
    },
};
use oasis_contract_sdk_storage::{
    cell::{ConfidentialCell, PublicCell},
    map::{ConfidentialMap, PublicMap},
};

/// All possible errors that can be returned by the contract.
///
/// Each error is a triplet of (module, code, message) which allows it to be both easily
/// human readable and also identifyable programmatically.
#[derive(Debug, thiserror::Error, sdk::Error)]
pub enum Error {
    #[error("bad request")]
    #[sdk_error(code = 1)]
    BadRequest,

    #[error("query failed")]
    #[sdk_error(code = 2)]
    QueryFailed,

    #[error("subcall failed: {0}")]
    #[sdk_error(code = 3)]
    SubcallFailed(String),

    #[error("upgrade not allowed (pre)")]
    #[sdk_error(code = 4)]
    UpgradeNotAllowedPre,

    #[error("upgrade not allowed (post)")]
    #[sdk_error(code = 5)]
    UpgradeNotAllowedPost,
}

/// All possible events that can be returned by the contract.
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Hello(String),
}

/// All possible requests that the contract can handle.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    #[cbor(rename = "instantiate")]
    Instantiate { initial_counter: u64 },

    #[cbor(rename = "say_hello")]
    SayHello { who: String },

    #[cbor(rename = "call_self")]
    CallSelf { once: bool },

    #[cbor(rename = "increment_counter")]
    IncrementCounter,

    #[cbor(rename = "instantiate_oas20")]
    InstantiateOas20 {
        code_id: CodeId,
        token_instantiation: TokenInstantiation,
    },

    #[cbor(rename = "ecdsa_recover")]
    ECDSARecover { input: Vec<u8> },

    #[cbor(rename = "signature_verify")]
    SignatureVerify {
        kind: u32,
        key: Vec<u8>,
        context: Vec<u8>,
        message: Vec<u8>,
        signature: Vec<u8>,
    },

    #[cbor(rename = "x25519_derive_symmetric")]
    X25519DeriveSymmetric {
        public_key: Vec<u8>,
        private_key: Vec<u8>,
    },

    #[cbor(rename = "deoxysii_seal")]
    DeoxysIISeal {
        key: Vec<u8>,
        nonce: Vec<u8>,
        message: Vec<u8>,
        additional_data: Vec<u8>,
    },

    #[cbor(rename = "deoxysii_open")]
    DeoxysIIOpen {
        key: Vec<u8>,
        nonce: Vec<u8>,
        message: Vec<u8>,
        additional_data: Vec<u8>,
    },

    #[cbor(rename = "random_bytes")]
    RandomBytes { count: u32 },

    #[cbor(rename = "invalid_storage_call")]
    InvalidStorageCall,

    #[cbor(rename = "query_address")]
    QueryAddress,

    #[cbor(rename = "query_block_info")]
    QueryBlockInfo,

    #[cbor(rename = "query_accounts")]
    QueryAccounts,

    #[cbor(rename = "query_ro")]
    QueryReadOnly,

    #[cbor(rename = "upgrade_proceed")]
    UpgradeProceed,

    #[cbor(rename = "upgrade_fail_pre")]
    UpgradeFailPre,

    #[cbor(rename = "upgrade_fail_post")]
    UpgradeFailPost,

    #[cbor(embed)]
    Oas20(ReceiverRequest),
}

/// All possible responses that the contract can return.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub enum Response {
    #[cbor(rename = "hello")]
    Hello { greeting: String },

    #[cbor(rename = "instantiate_oas20")]
    InstantiateOas20 {
        instance_id: InstanceId,
        data: String,
    },

    #[cbor(rename = "ecdsa_recover")]
    ECDSARecover { output: [u8; 65] },

    #[cbor(rename = "signature_verify")]
    SignatureVerify { result: bool },

    #[cbor(rename = "x25519_derive_symmetric")]
    X25519DeriveSymmetric { output: [u8; 32] },

    #[cbor(rename = "random_bytes")]
    RandomBytes { output: Vec<u8> },

    #[cbor(rename = "deoxysii_response")]
    DeoxysIIResponse { error: bool, output: Vec<u8> },

    #[cbor(rename = "empty")]
    Empty,
}

/// The contract type.
pub struct HelloWorld;

/// Storage cell for the public counter.
const COUNTER: PublicCell<u64> = PublicCell::new(b"counter");

/// Storage cell for the confidential counter.
const CONFIDENTIAL_COUNTER: ConfidentialCell<u64> = ConfidentialCell::new(b"confidential_counter");

/// Storage for the call public timestamps.
const TIMESTAMPS: PublicMap<String, u64> = PublicMap::new(b"timestamps");

/// Storage for the call confidential timestamps.
const CONFIDENTIAL_TIMESTAMPS: ConfidentialMap<String, u64> =
    ConfidentialMap::new(b"confidential_timestamps");

impl HelloWorld {
    /// Increment the counter and return the previous value.
    fn increment_counter<C: sdk::Context>(ctx: &mut C, inc: u64) -> u64 {
        let counter = COUNTER.get(ctx.public_store()).unwrap_or_default();
        COUNTER.set(ctx.public_store(), counter + inc);

        let confidential_counter = CONFIDENTIAL_COUNTER
            .get(ctx.confidential_store())
            .unwrap_or_default();
        if confidential_counter != counter {
            return u64::MAX;
        }
        CONFIDENTIAL_COUNTER.set(ctx.confidential_store(), confidential_counter + inc);

        counter
    }

    /// Stores the counter value for the given user.
    fn store_timestamp<C: sdk::Context>(ctx: &mut C, who: String, timestamp: u64) {
        TIMESTAMPS.insert(ctx.public_store(), who.clone(), timestamp);
        CONFIDENTIAL_TIMESTAMPS.insert(ctx.confidential_store(), who, timestamp);
    }
}

// Implementation of the sdk::Contract trait is required in order for the type to be a contract.
impl sdk::Contract for HelloWorld {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    fn instantiate<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<(), Error> {
        // This method is called during the contracts.Instantiate call when the contract is first
        // instantiated. It can be used to initialize the contract state.
        match request {
            // We require the caller to always pass the Instantiate request.
            Request::Instantiate { initial_counter } => {
                // Initialize counter to specified value.
                COUNTER.set(ctx.public_store(), initial_counter);
                CONFIDENTIAL_COUNTER.set(ctx.confidential_store(), initial_counter);

                Ok(())
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Call call. It is supposed to handle the request
        // and return a response.
        match request {
            Request::SayHello { who } => {
                // Increment the counter and retrieve the previous value.
                let counter = Self::increment_counter(ctx, 1);

                // Store current counter value as timestamp.
                Self::store_timestamp(ctx, who.clone(), counter);

                // Emit a test event.
                ctx.emit_event(Event::Hello("world".to_string()));

                // Return the greeting as a response.
                Ok(Response::Hello {
                    greeting: format!("hello {} ({})", who, counter),
                })
            }
            Request::CallSelf { once } => {
                // This request is used in tests to test subcalls.
                use cbor::cbor_map;

                let args = if once {
                    // Only one subcall.
                    cbor::cbor_map! {
                        "say_hello" => cbor::cbor_map! {
                            "who" => cbor::cbor_text!("subcall"),
                        },
                    }
                } else {
                    // This request is used in tests to attempt to trigger infinite recursion
                    // through subcalls as it invokes the same method again and again. In reality
                    // propagation should stop when running out of gas or reaching the maximum
                    // subcall depth.
                    cbor::cbor_map! {
                        "call_self" => cbor::cbor_map! {
                            "once" => cbor::cbor_bool!(false),
                        },
                    }
                };

                // Emit a message through which we instruct the runtime to make a call on the
                // contract's behalf. In this case we use it to call into our own contract.
                //
                // Any results from these calls will be processed in `handle_reply` below.
                ctx.emit_message(Message::Call {
                    id: 0,
                    reply: NotifyReply::Always,
                    method: "contracts.Call".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(ctx.instance_id().as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(args)),
                        "tokens" => cbor::cbor_array![],
                    },
                    max_gas: None,
                    data: None,
                });
                Ok(Response::Empty)
            }
            Request::IncrementCounter => {
                // Just increment the counter and return an empty response.
                Self::increment_counter(ctx, 1);

                Ok(Response::Empty)
            }
            Request::InstantiateOas20 {
                code_id,
                token_instantiation,
            } => {
                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: 42,
                    reply: NotifyReply::Always,
                    method: "contracts.Instantiate".to_string(),
                    body: cbor::cbor_map! {
                        "code_id" => cbor::cbor_int!(code_id.as_u64() as i64),
                        "upgrades_policy" => cbor::cbor_map!{
                            "everyone" => cbor::cbor_map!{},
                        },
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(Oas20Request::Instantiate(token_instantiation)),
                        )),
                        // Forward any deposited native tokens, as an example of sending native tokens.
                        "tokens" => cbor::to_value(ctx.deposited_tokens().to_vec()),
                    },
                    max_gas: None,
                    data: Some(cbor::to_value("some test data".to_string())),
                });

                Ok(Response::Empty)
            }
            Request::ECDSARecover { input } => {
                let output = ctx.env().ecdsa_recover(&input);

                Ok(Response::ECDSARecover { output })
            }
            Request::SignatureVerify {
                kind,
                key,
                context,
                message,
                signature,
            } => {
                let kind: SignatureKind = kind.try_into().map_err(|_| Error::BadRequest)?;
                let result = match kind {
                    SignatureKind::Ed25519 => ctx
                        .env()
                        .signature_verify_ed25519(&key, &message, &signature),
                    SignatureKind::Secp256k1 => ctx
                        .env()
                        .signature_verify_secp256k1(&key, &message, &signature),
                    SignatureKind::Sr25519 => ctx
                        .env()
                        .signature_verify_sr25519(&key, &context, &message, &signature),
                };

                Ok(Response::SignatureVerify { result })
            }
            Request::X25519DeriveSymmetric {
                public_key,
                private_key,
            } => {
                let output = ctx.env().x25519_derive_symmetric(&public_key, &private_key);
                Ok(Response::X25519DeriveSymmetric { output })
            }
            Request::DeoxysIISeal {
                key,
                nonce,
                message,
                additional_data,
            } => {
                let output = ctx
                    .env()
                    .deoxysii_seal(&key, &nonce, &message, &additional_data);
                let error = output.is_err();
                let output = output.unwrap_or_else(|_| Vec::new());
                Ok(Response::DeoxysIIResponse { error, output })
            }
            Request::DeoxysIIOpen {
                key,
                nonce,
                message,
                additional_data,
            } => {
                let output = ctx
                    .env()
                    .deoxysii_open(&key, &nonce, &message, &additional_data);
                let error = output.is_err();
                let output = output.unwrap_or_else(|_| Vec::new());
                Ok(Response::DeoxysIIResponse { error, output })
            }
            Request::RandomBytes { count } => {
                let mut buf = vec![0; count as usize];
                let bytes_written = ctx.env().random_bytes(&[], &mut buf);
                buf.truncate(bytes_written);
                Ok(Response::RandomBytes { output: buf })
            }
            Request::InvalidStorageCall => {
                // Generate an invalid storage call.
                let big_value = vec![0x80; 70]; // Limit in tests is 64.
                ctx.public_store().insert(&big_value, b"boom");

                Ok(Response::Empty)
            }
            Request::QueryAddress => {
                let address = ctx.env().address_for_instance(ctx.instance_id());

                Ok(Response::Hello {
                    greeting: format!("my address is: {}", address.to_bech32()),
                })
            }
            Request::QueryBlockInfo => match ctx.env().query(QueryRequest::BlockInfo) {
                QueryResponse::BlockInfo {
                    round,
                    epoch,
                    timestamp,
                    ..
                } => Ok(Response::Hello {
                    greeting: format!("round: {} epoch: {} timestamp: {}", round, epoch, timestamp),
                }),

                _ => Err(Error::QueryFailed),
            },
            Request::QueryAccounts => match ctx.env().query(AccountsQuery::Balance {
                address: *ctx.instance_address(),
                denomination: token::Denomination::NATIVE,
            }) {
                QueryResponse::Accounts(AccountsResponse::Balance { balance }) => {
                    Ok(Response::Hello {
                        greeting: format!("my native balance is: {}", balance as u64),
                    })
                }

                _ => Err(Error::QueryFailed),
            },
            Request::QueryReadOnly => {
                let counter = COUNTER.get(ctx.public_store()).unwrap_or_default();

                Ok(Response::Hello {
                    greeting: format!(
                        "hello {} ({}, {})",
                        ctx.caller_address().to_bech32(),
                        counter,
                        ctx.is_read_only()
                    ),
                })
            }
            // Handle receiving Oas20 tokens.
            Request::Oas20(ReceiverRequest::Receive {
                sender: _,
                amount: _,
                data,
            }) => {
                // Just increment the counter by the amount specified in the accompanying data.
                let inc: u64 = cbor::from_value(data).unwrap();
                Self::increment_counter(ctx, inc);

                Ok(Response::Empty)
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn query<C: sdk::Context>(_ctx: &mut C, _request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Query query. It is supposed to handle the
        // request and return a response.
        Err(Error::BadRequest)
    }

    fn handle_reply<C: sdk::Context>(
        _ctx: &mut C,
        reply: Reply,
    ) -> Result<Option<Self::Response>, Error> {
        // This method is called to handle any replies for emitted messages.
        match reply {
            Reply::Call { id, result, .. } if id == 0 => {
                match result {
                    CallResult::Failed { module, code } => {
                        // Propagate all failures.
                        Err(Error::SubcallFailed(format!(
                            "module={} code={}",
                            module, code
                        )))
                    }
                    CallResult::Ok(val) => {
                        // Replace result with the subcall's result.
                        let val: Vec<u8> = cbor::from_value(val).unwrap();
                        let val = cbor::from_slice(&val).unwrap();
                        Ok(Some(val))
                    }
                }
            }
            Reply::Call { id, result, data } if id == 42 => {
                let data = cbor::from_value(data.unwrap()).unwrap();

                let result: InstantiateResult = match result {
                    CallResult::Ok(val) => Ok(cbor::from_value(val).unwrap()),
                    _ => Err(Error::QueryFailed),
                }?;
                Ok(Some(Response::InstantiateOas20 {
                    instance_id: result.id,
                    data,
                }))
            }

            _ => Err(Error::BadRequest),
        }
    }

    fn pre_upgrade<C: sdk::Context>(_ctx: &mut C, request: Self::Request) -> Result<(), Error> {
        // This method is called on the old contract code before an upgrade is supposed to happen.
        // In case it returns an error, the upgrade will be rejected.
        match request {
            // Allow any upgrade if request is right.
            Request::UpgradeProceed | Request::UpgradeFailPost => Ok(()),

            // Reject all other upgrades.
            _ => Err(Error::UpgradeNotAllowedPre),
        }
    }

    fn post_upgrade<C: sdk::Context>(_ctx: &mut C, request: Self::Request) -> Result<(), Error> {
        // This method is called on the new contract code after the code has been upgraded. In case
        // it returns an error, the upgrade will be rejected.
        match request {
            // Allow any upgrade if request is right.
            Request::UpgradeProceed => Ok(()),

            // Reject all other upgrades.
            _ => Err(Error::UpgradeNotAllowedPost),
        }
    }
}

// Create the required WASM exports required for the contract to be runnable.
sdk::create_contract!(HelloWorld);

// We define some simple contract tests below.
#[cfg(test)]
mod test {
    use oasis_contract_sdk::{testing::MockContext, types::ExecutionContext, Contract};

    use super::*;

    #[test]
    fn test_hello() {
        // Create a mock execution context with default values.
        let mut ctx: MockContext = ExecutionContext::default().into();

        // Instantiate the contract.
        HelloWorld::instantiate(
            &mut ctx,
            Request::Instantiate {
                initial_counter: 11,
            },
        )
        .expect("instantiation should work");

        // Dispatch the SayHello message.
        let rsp = HelloWorld::call(
            &mut ctx,
            Request::SayHello {
                who: "unit test".to_string(),
            },
        )
        .expect("SayHello call should work");

        // Make sure the greeting is correct.
        assert_eq!(
            rsp,
            Response::Hello {
                greeting: "hello unit test (11)".to_string()
            }
        );

        // Dispatch another SayHello message.
        let rsp = HelloWorld::call(
            &mut ctx,
            Request::SayHello {
                who: "second call".to_string(),
            },
        )
        .expect("SayHello call should work");

        // Make sure the greeting is correct.
        assert_eq!(
            rsp,
            Response::Hello {
                greeting: "hello second call (12)".to_string()
            }
        );
    }
}
