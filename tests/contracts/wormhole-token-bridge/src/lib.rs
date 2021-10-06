//! Oasis Wormhole Token Bridge Contract.
// As described in https://github.com/certusone/wormhole/blob/dev.v2/design/0003_token_bridge.md
#![feature(wasm_abi)]

extern crate alloc;

use std::cmp::max;

use oasis_contract_sdk::{
    self as sdk,
    env::Env,
    types::{
        env::{QueryRequest, QueryResponse},
        message::{CallResult, Message, NotifyReply, Reply},
        modules::contracts::InstantiateResult,
    },
};
use oasis_contract_sdk_storage::{
    cell::Cell,
    map::{Int, Map},
};
use oasis_contract_sdk_types::InstanceId;

use oasis_contract_sdk_oas20_types as oas20;
use oasis_oas20_wrapped_types as oas20wrapped;
use oasis_wormhole_types as wormhole;

pub mod spec;
#[cfg(test)]
mod test;
pub mod types;
use types::{Error, Event, Request, Response};

/// The contract type.
pub struct WormholeTokenBridge;

const MODULE_NAME: &str = "TokenBridge";

/// Storage cell for the state.
const CONFIG: Cell<types::Configuration> = Cell::new(b"configuration");
/// Storage map of previously submitted VAAs.
const VAAS: Map<&[u8], bool> = Map::new(b"vaas");
/// Storage map of registered bridge chain IDs and corresponding contract addresses.
const REGISTERED_CHAINS: Map<Int<u16>, wormhole::spec::Address> = Map::new(b"registered_chains");

/// Storage map of wrapped asset IDs and corresponding contract addresses.
const WRAPPED_ASSET_IDS: Map<[u8; 34], InstanceId> = Map::new(b"wrapped_asset_ids");
/// Storage map of wrapped asset addresses and corresponding asset IDs.
const WRAPPED_ASSET_ADDRS: Map<Int<u64>, [u8; 34]> = Map::new(b"wrapped_asset_addresses");

/// Locked assets stores the native asset locked token amounts.
const LOCKED_ASSETS: Map<Int<u64>, u128> = Map::new(b"locked_assets");

// Request::AttestMeta -> instantiate Wrapped-OAS20.
const REPLY_ID_ATTEST_META_WRAPPED_INIT: u64 = 0;

// Request::SubmitVAA -> wormhole::Query(ParseVAA).
const REPLY_ID_SUBMIT_VAA_PARSE_VAA: u64 = 1;
// Request::SubmitVAA -> wormhole::Query(ParseVAA) (wrapped).
const REPLY_ID_INBOUND_TRANSFER_WRAPPED: u64 = 2;
// Request::SubmitVAA -> wormhole::Query(ParseVAA) (OAS20).
const REPLY_ID_INBOUND_TRANSFER_OAS20: u64 = 3;
// Request::SubmitVAA -> wormhole::Query(ParseVAA) (OAS20) -> OAS20 asset transfer.
const REPLY_ID_INBOUND_TRANSFER_OAS20_TRANSFER: u64 = 4;

// Request::CreateAssetMeta -> Query token.
const REPLY_ID_CREATE_ASSET_META_QUERY_TOKEN: u64 = 5;
// Request::CreateAssetMeta -> Query token -> Wormhole Post message.
const REPLY_ID_CREATE_ASSET_META_POST_MESSAGE: u64 = 6;

// Request::InitiateTransfer -> Burn Wrapped.
const REPLY_ID_OUTBOUND_TRANSFER_WRAPPED: u64 = 7;
// Request::InitiateTransfer -> Burn Wrapped -> Query token info.
const REPLY_ID_OUTBOUND_TRANSFER_WRAPPED_INFO: u64 = 8;
// Request::InitiateTransfer -> Burn Wrapped -> Query token info -> Wormhole Post Message.
const REPLY_ID_OUTBOUND_TRANSFER_POST_MESSAGE: u64 = 9;

// Request::InitiateTransfer -> Query OAS20 token info.
const REPLY_ID_OUTBOUND_TRANSFER_OAS20: u64 = 10;
// Request::InitiateTransfer -> Query OAS20 token info -> OAS20 Withdraw.
const REPLY_ID_OUTBOUND_TRANSFER_OAS20_WITHDRAW: u64 = 11;

impl WormholeTokenBridge {
    fn handle_governance_payload<C: sdk::Context>(
        ctx: &mut C,
        vaa: wormhole::spec::ParsedVAA,
    ) -> Result<(), Error> {
        let gov_packet = spec::GovernancePacket::deserialize(&vaa.payload)?;

        if gov_packet.module != MODULE_NAME {
            return Err(Error::InvalidVAAModule);
        }

        if gov_packet.chain != 0 && gov_packet.chain != wormhole::spec::OASIS_CHAIN_ID {
            return Err(Error::InvalidVAAChainId);
        }

        match gov_packet.action {
            spec::GovernanceAction::RegisterChain => {
                // Register the token bridge contract (emitter address) for a foreign chain.
                let spec::RegisterChain {
                    emitter_chain_id,
                    emitter_address,
                } = spec::RegisterChain::deserialize(&gov_packet.payload)?;

                if REGISTERED_CHAINS
                    .get(ctx.public_store(), emitter_chain_id.into())
                    .is_some()
                {
                    return Err(Error::ChainAlreadyRegistered);
                }

                REGISTERED_CHAINS.insert(
                    ctx.public_store(),
                    emitter_chain_id.into(),
                    emitter_address,
                );

                Ok(())
            }
        }
    }

    fn handle_inbound_transfer<C: sdk::Context>(
        ctx: &mut C,
        emitter_chain: u16,
        emitter_address: wormhole::spec::Address,
        data: &[u8],
    ) -> Result<(), Error> {
        // Inbound transfer.
        let transfer_info = spec::TransferInfo::deserialize(data)?;

        let expected_address = REGISTERED_CHAINS
            .get(ctx.public_store(), emitter_chain.into())
            .ok_or(Error::ChainNotRegistered)?;
        if expected_address != emitter_address {
            return Err(Error::InvalidEmitterAddress);
        }

        if wormhole::spec::OASIS_CHAIN_ID != transfer_info.recipient_chain {
            return Err(Error::TransferNotForOasis);
        }

        // Only u128 supported.
        let (amount_overflow, mut amount) = transfer_info.amount;
        let (fee_overflow, fee) = transfer_info.fee;

        // Subtract fee from the amount.
        amount = amount
            .checked_sub(fee)
            .ok_or(Error::TransferFeeGreaterThanAmount)?;

        if amount_overflow != 0 || fee_overflow != 0 {
            return Err(Error::AmountTooHigh);
        }

        let mut data = types::InboundTransferData {
            amount,
            fee,
            recipient: transfer_info
                .recipient
                .as_oasis_address()
                .map_err(|_| Error::InvalidVAAPayload)?,
            ..Default::default()
        };

        match transfer_info.token_chain {
            wormhole::spec::OASIS_CHAIN_ID => {
                // Token chain is oasis, this is an OAS-20 token.

                let token_instance_id = transfer_info
                    .token_address
                    .as_instance_id()
                    .map_err(|_| Error::InvalidVAAPayload)?;
                data.asset = token_instance_id;

                // Query token info.
                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: REPLY_ID_INBOUND_TRANSFER_OAS20,
                    reply: NotifyReply::Always,
                    method: "contracts.Query".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(token_instance_id.as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(oas20::Request::TokenInformation),
                        )),
                        "tokens" => cbor::cbor_array![],
                    },
                    max_gas: None,
                    data: Some(cbor::to_value(data)),
                });
            }
            _ => {
                // Token chain is not oasis, this must be a wrapped asset.
                // Mint wrapped tokens.
                let asset_id =
                    spec::wrapped_asset_id(transfer_info.token_chain, &transfer_info.token_address);

                // Ensure wrapped asset deployed.
                let wrapped_instance_id = WRAPPED_ASSET_IDS
                    .get(ctx.public_store(), asset_id)
                    .ok_or(Error::WrappedAssetNotDeployed)?;

                data.asset = wrapped_instance_id;

                let to = data.recipient;

                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: REPLY_ID_INBOUND_TRANSFER_WRAPPED,
                    reply: NotifyReply::Always,
                    method: "contracts.Call".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(wrapped_instance_id.as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(oas20wrapped::Request::Oas20(oas20::Request::Mint{to, amount})),
                        )),
                        "tokens" => cbor::cbor_array![],
                    },
                    max_gas: None,
                    data: Some(cbor::to_value(data.clone())),
                });
                if fee != 0 {
                    // Transfer fee to the caller.
                    ctx.emit_message(Message::Call {
                        id: REPLY_ID_INBOUND_TRANSFER_WRAPPED,
                        reply: NotifyReply::Always,
                        method: "contracts.Call".to_string(),
                        body: cbor::cbor_map! {
                            "id" => cbor::cbor_int!(wrapped_instance_id.as_u64() as i64),
                            "data" => cbor::cbor_bytes!(cbor::to_vec(
                                cbor::to_value(oas20wrapped::Request::Oas20(oas20::Request::Mint{to: *ctx.caller_address(), amount: fee})),
                            )),
                            "tokens" => cbor::cbor_array![],
                        },
                        max_gas: None,
                        data: Some(cbor::to_value(data)),
                    });
                }
            }
        }

        Ok(())
    }

    fn handle_attest_meta<C: sdk::Context>(
        ctx: &mut C,
        emitter_chain: u16,
        emitter_address: wormhole::spec::Address,
        data: &[u8],
    ) -> Result<(), Error> {
        let meta = spec::AssetMeta::deserialize(data)?;

        let expected_address = REGISTERED_CHAINS
            .get(ctx.public_store(), emitter_chain.into())
            .ok_or(Error::ChainNotRegistered)?;
        if expected_address != emitter_address {
            return Err(Error::InvalidEmitterAddress);
        }

        if wormhole::spec::OASIS_CHAIN_ID == meta.token_chain {
            return Err(Error::AttestingNativeAsset);
        }

        let cfg = CONFIG.get(ctx.public_store()).unwrap();

        let asset_id = spec::wrapped_asset_id(meta.token_chain, &meta.token_address);
        if WRAPPED_ASSET_IDS
            .get(ctx.public_store(), asset_id)
            .is_some()
        {
            return Err(Error::AssetAlreadyAttested);
        }
        let data = types::AttestMetaData { asset: asset_id };

        // Instantiate Wrapped OAS20 token.
        let our_address = ctx.env().address_for_instance(ctx.instance_id());
        let token_instantiation = oas20::TokenInstantiation {
            name: meta.name,
            symbol: meta.symbol,
            decimals: meta.decimals,
            initial_balances: Vec::new(),
            minting: Some(oas20::MintingInformation {
                minter: our_address,
                cap: None,
            }),
        };

        use cbor::cbor_map;
        ctx.emit_message(Message::Call {
            id: REPLY_ID_ATTEST_META_WRAPPED_INIT,
            reply: NotifyReply::Always,
            method: "contracts.Instantiate".to_string(),
            body: cbor::cbor_map! {
                "code_id" => cbor::cbor_int!(cfg.wrapped_asset_code_id.as_u64() as i64),
                "upgrades_policy" => cbor::cbor_map!{
                    "nobody" => cbor::cbor_map!{},
                },
                "data" => cbor::cbor_bytes!(cbor::to_vec(
                    cbor::to_value(oas20wrapped::Request::Instantiate{
                        token_instantiation,
                        asset_chain_id: meta.token_chain,
                        asset_address: meta.token_address,

                    }),
                )),
                "tokens" => cbor::cbor_array![],
            },
            max_gas: None,
            data: Some(cbor::to_value(data)),
        });

        // Continues in handle reply.

        Ok(())
    }
}

impl sdk::Contract for WormholeTokenBridge {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    fn instantiate<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<(), Error> {
        match request {
            Request::Instantiate(configuration) => {
                CONFIG.set(ctx.public_store(), configuration);

                Ok(())
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        match request {
            // Outbound transfer.
            Request::InitiateTransfer {
                asset,
                amount,
                recipient_chain,
                recipient,
                fee,
                nonce,
            } => {
                if recipient_chain == wormhole::spec::OASIS_CHAIN_ID {
                    return Err(Error::TransferRecipientOasisChain);
                }
                if amount == 0 {
                    return Err(Error::AmountTooLow);
                }
                if fee > amount {
                    return Err(Error::TransferFeeGreaterThanAmount);
                }

                let data = types::OutboundTransferData {
                    amount,
                    fee,
                    recipient,
                    recipient_chain,
                    asset,
                    nonce,
                    deposited_tokens: ctx.deposited_tokens().to_vec(),
                };

                match WRAPPED_ASSET_ADDRS.get(ctx.public_store(), asset.as_u64().into()) {
                    Some(_) => {
                        // This is a wrapped asset, burn tokens.

                        use cbor::cbor_map;
                        ctx.emit_message(Message::Call {
                            id: REPLY_ID_OUTBOUND_TRANSFER_WRAPPED,
                            reply: NotifyReply::Always,
                            method: "contracts.Call".to_string(),
                            body: cbor::cbor_map! {
                                "id" => cbor::cbor_int!(asset.as_u64() as i64),
                                "data" => cbor::cbor_bytes!(cbor::to_vec(
                                    cbor::to_value(oas20wrapped::Request::BurnFrom{from: ctx.caller_address().to_owned(), amount}),
                                )),
                                "tokens" => cbor::cbor_array![],
                            },
                            max_gas: None,
                            data: Some(cbor::to_value(
                                data,
                            )),
                        });

                        // Continues in handle reply.
                        Ok(Response::Empty)
                    }
                    None => {
                        // Not wrapped, this is likely an OAS-20 token - we will withdraw tokens.

                        use cbor::cbor_map;
                        ctx.emit_message(Message::Call {
                            id: REPLY_ID_OUTBOUND_TRANSFER_OAS20,
                            reply: NotifyReply::Always,
                            method: "contracts.Query".to_string(),
                            body: cbor::cbor_map! {
                                "id" => cbor::cbor_int!(asset.as_u64() as i64),
                                "data" => cbor::cbor_bytes!(cbor::to_vec(
                                    cbor::to_value(oas20::Request::TokenInformation),
                                )),
                                "tokens" => cbor::cbor_array![],
                            },
                            max_gas: None,
                            data: Some(cbor::to_value(data)),
                        });

                        Ok(Response::Empty)
                    }
                }
            }
            Request::SubmitVAA { data } => {
                let cfg = CONFIG.get(ctx.public_store()).unwrap();

                // Query block timestamp.
                let block_time = match ctx.env().query(QueryRequest::BlockInfo) {
                    QueryResponse::BlockInfo { timestamp, .. } => Ok(timestamp),
                    _ => Err(Error::QueryFailed),
                }?;

                // Query wormhole contract.
                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: REPLY_ID_SUBMIT_VAA_PARSE_VAA,
                    reply: NotifyReply::Always,
                    method: "contracts.Query".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(cfg.wormhole_contract.as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(wormhole::Request::VerifyVAA{vaa: data, block_time}),
                        )),
                        "tokens" => cbor::cbor_array![],
                    },
                    max_gas: None,
                    data: None,
                });

                // Continues in handle reply.

                Ok(Response::Empty)
            }
            Request::CreateAssetMeta {
                asset_instance_id,
                nonce,
            } => {
                // The metadata of a token can be attested by calling CreateAssetMeta on its respective native chain,
                // which will produce an AssetMeta wormhole message.
                // This message can be used to attest state and initialize a WrappedAsset
                // on any chain in the wormhole network using the details.
                // A token is identified by the tuple (chain_id, chain_address) and metadata should be mapped to this identifier.
                // A wrapped asset may only ever be created once for a given identifier and not updated.

                let data = types::CreateAssetMetaData {
                    asset: asset_instance_id,
                    nonce,
                    deposited_tokens: ctx.deposited_tokens().to_vec(),
                };

                // Query OAS20 token contract.
                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: REPLY_ID_CREATE_ASSET_META_QUERY_TOKEN,
                    reply: NotifyReply::Always,
                    method: "contracts.Query".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(asset_instance_id.as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(oas20::Request::TokenInformation),
                        )),
                        "tokens" => cbor::cbor_array![],
                    },
                    max_gas: None,
                    data: Some(cbor::to_value(data)),
                });

                // Continues in handle reply.

                Ok(Response::Empty)
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn query<C: sdk::Context>(_ctx: &mut C, _request: Request) -> Result<Response, Error> {
        Err(Error::BadRequest)
    }

    fn handle_reply<C: sdk::Context>(
        ctx: &mut C,
        reply: Reply,
    ) -> Result<Option<Self::Response>, Error> {
        // This method is called to handle any replies for emitted messages.
        match reply {
            Reply::Call {
                id, result, data, ..
            } if id == REPLY_ID_ATTEST_META_WRAPPED_INIT => {
                let data: types::AttestMetaData = cbor::from_value(data.unwrap()).unwrap();

                let result: InstantiateResult = match result {
                    CallResult::Ok(val) => Ok(cbor::from_value(val).unwrap()),
                    _ => Err(Error::BadRequest),
                }?;

                // Save WormholeId->Instnace_id, Instance_id->WormholeId.
                WRAPPED_ASSET_IDS.insert(ctx.public_store(), data.asset, result.id);
                WRAPPED_ASSET_ADDRS.insert(
                    ctx.public_store(),
                    result.id.as_u64().into(),
                    data.asset,
                );

                // Emit register asset event.
                ctx.emit_event(Event::AssetRegistered {
                    contract_instance_id: result.id,
                });

                // TODO: maybe also return register chain info.
                Ok(None)
            }
            Reply::Call { id, result, .. } if id == REPLY_ID_SUBMIT_VAA_PARSE_VAA => {
                let cfg = CONFIG.get(ctx.public_store()).unwrap();

                let vaa: wormhole::spec::ParsedVAA = match result {
                    CallResult::Ok(val) => Ok(cbor::from_value(val).unwrap()),
                    _ => Err(Error::BadRequest),
                }?;

                // Check if this VAA was already executed.
                if VAAS.get(ctx.public_store(), &vaa.hash).unwrap_or_default() {
                    return Err(Error::VAAAlreadyExecuted);
                }
                // Store VAA as seen.
                VAAS.insert(ctx.public_store(), &vaa.hash, true);

                if cfg.governance_chain != vaa.emitter_chain
                    || cfg.governance_address != vaa.emitter_address
                {
                    // Handle governance VAA.
                    return Self::handle_governance_payload(ctx, vaa).map(|_| None);
                }

                let message = spec::TokenBridgeMessage::deserialize(&vaa.payload)?;

                match message.action {
                    spec::TokenBridgeAction::Transfer => Self::handle_inbound_transfer(
                        ctx,
                        vaa.emitter_chain,
                        vaa.emitter_address,
                        &message.payload,
                    ),
                    spec::TokenBridgeAction::AttestMeta => Self::handle_attest_meta(
                        ctx,
                        vaa.emitter_chain,
                        vaa.emitter_address,
                        &message.payload,
                    ),
                }
                .map(|_| None)
            }
            Reply::Call { id, result, .. } if id == REPLY_ID_INBOUND_TRANSFER_WRAPPED => {
                if !result.is_success() {
                    return Err(Error::TransferFailed);
                }

                Ok(None)
            }
            Reply::Call {
                id, result, data, ..
            } if id == REPLY_ID_INBOUND_TRANSFER_OAS20 => {
                let transfer_data: types::InboundTransferData =
                    cbor::from_value(data.clone().unwrap()).unwrap();

                let mut amount = transfer_data.amount;
                let mut fee = transfer_data.fee;

                if let oas20::Response::TokenInformation { token_information } = match result {
                    CallResult::Ok(val) => Ok(cbor::from_value(val).unwrap()),
                    _ => Err(Error::CreateAssetMetaFailed),
                }? {
                    // Update locked asset state.
                    let mut locked = LOCKED_ASSETS
                        .get(ctx.public_store(), transfer_data.asset.as_u64().into())
                        .unwrap();
                    locked = locked
                        .checked_sub(transfer_data.amount + transfer_data.fee) // fee was already subtracted form amount.
                        .unwrap();
                    LOCKED_ASSETS.insert(
                        ctx.public_store(),
                        transfer_data.asset.as_u64().into(),
                        locked,
                    );

                    // De-normalize the 8-decimals wormhole normalization.
                    let decimals = token_information.decimals;
                    let multiplier = 10u128.pow((max(decimals, 8u8) - 8u8) as u32);
                    amount = amount.checked_mul(multiplier).unwrap();
                    fee = fee.checked_mul(multiplier).unwrap();

                    let to = transfer_data.recipient;
                    // Unlock tokens.
                    use cbor::cbor_map;
                    ctx.emit_message(Message::Call {
                        id: REPLY_ID_INBOUND_TRANSFER_OAS20_TRANSFER,
                        reply: NotifyReply::Always,
                        method: "contracts.Call".to_string(),
                        body: cbor::cbor_map! {
                            "id" => cbor::cbor_int!(transfer_data.asset.as_u64() as i64),
                            "data" => cbor::cbor_bytes!(cbor::to_vec(
                                cbor::to_value(oas20::Request::Transfer{ to, amount }),
                            )),
                            "tokens" => cbor::cbor_array![],
                        },
                        max_gas: None,
                        data: data.clone(),
                    });

                    if fee != 0 {
                        // Transfer fee to the caller.
                        ctx.emit_message(Message::Call {
                            id: REPLY_ID_INBOUND_TRANSFER_OAS20_TRANSFER,
                            reply: NotifyReply::Always,
                            method: "contracts.Call".to_string(),
                            body: cbor::cbor_map! {
                                "id" => cbor::cbor_int!(transfer_data.asset.as_u64() as i64),
                                "data" => cbor::cbor_bytes!(cbor::to_vec(
                                    cbor::to_value(oas20::Request::Transfer{ to: *ctx.caller_address(), amount: fee }),
                                )),
                                "tokens" => cbor::cbor_array![],
                            },
                            max_gas: None,
                            data,
                        });
                    }

                    Ok(None)
                } else {
                    Err(Error::TransferFailed)
                }
            }
            Reply::Call { id, result, .. } if id == REPLY_ID_INBOUND_TRANSFER_OAS20_TRANSFER => {
                if !result.is_success() {
                    return Err(Error::TransferFailed);
                }

                Ok(None)
            }
            Reply::Call {
                id, result, data, ..
            } if id == REPLY_ID_CREATE_ASSET_META_QUERY_TOKEN => {
                let asset_data: types::CreateAssetMetaData =
                    cbor::from_value(data.clone().unwrap()).unwrap();

                let cfg = CONFIG.get(ctx.public_store()).unwrap();

                if let oas20::Response::TokenInformation { token_information } = match result {
                    CallResult::Ok(val) => Ok(cbor::from_value(val).unwrap()),
                    _ => Err(Error::CreateAssetMetaFailed),
                }? {
                    let meta = spec::AssetMeta {
                        token_chain: wormhole::spec::OASIS_CHAIN_ID,
                        token_address: asset_data.asset.into(), // NOTE/TODO: we use instance ID as the address.
                        decimals: token_information.decimals,
                        symbol: token_information.symbol,
                        name: token_information.name,
                    };

                    let message = spec::TokenBridgeMessage {
                        action: spec::TokenBridgeAction::AttestMeta,
                        payload: meta.serialize()?,
                    };
                    let message = message.serialize();

                    // Post the message to the wormhole contract.
                    use cbor::cbor_map;
                    ctx.emit_message(Message::Call {
                        id: REPLY_ID_CREATE_ASSET_META_POST_MESSAGE,
                        reply: NotifyReply::Always,
                        method: "contracts.Call".to_string(),
                        body: cbor::cbor_map! {
                            "id" => cbor::cbor_int!(cfg.wormhole_contract.as_u64() as i64),
                            "data" => cbor::cbor_bytes!(cbor::to_vec(
                                cbor::to_value(wormhole::Request::PostMessage{message, nonce: asset_data.nonce}),
                            )),
                            "tokens" => cbor::to_value(asset_data.deposited_tokens), // Forward any native tokens received (fee) to the Wormhole chain.
                        },
                        max_gas: None,
                        data
                    });

                    Ok(None)
                } else {
                    Err(Error::CreateAssetMetaFailed)
                }
            }
            Reply::Call { id, result, .. } if id == REPLY_ID_CREATE_ASSET_META_POST_MESSAGE => {
                if !result.is_success() {
                    return Err(Error::CreateAssetMetaFailed);
                }

                Ok(None)
            }
            Reply::Call {
                id, result, data, ..
            } if id == REPLY_ID_OUTBOUND_TRANSFER_WRAPPED => {
                if !result.is_success() {
                    return Err(Error::TransferFailed);
                }

                let transfer_data: types::OutboundTransferData =
                    cbor::from_value(data.clone().unwrap()).unwrap();

                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: REPLY_ID_OUTBOUND_TRANSFER_WRAPPED_INFO,
                    reply: NotifyReply::Always,
                    method: "contracts.Query".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(transfer_data.asset.as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(oas20wrapped::Request::BridgeWrappedInfo),
                        )),
                        "tokens" => cbor::cbor_array![],
                    },
                    max_gas: None,
                    data,
                });

                Ok(None)
            }
            Reply::Call {
                id, result, data, ..
            } if id == REPLY_ID_OUTBOUND_TRANSFER_WRAPPED_INFO => {
                if !result.is_success() {
                    return Err(Error::TransferFailed);
                }

                let cfg = CONFIG.get(ctx.public_store()).unwrap();

                let transfer_data: types::OutboundTransferData =
                    cbor::from_value(data.clone().unwrap()).unwrap();

                if let oas20wrapped::Response::BridgeWrappedInfo { info } = match result {
                    CallResult::Ok(val) => Ok(cbor::from_value(val).unwrap()),
                    _ => Err(Error::TransferFailed),
                }? {
                    let transfer_info = spec::TransferInfo {
                        amount: (0, transfer_data.amount),
                        token_address: info.asset_address,
                        token_chain: info.asset_chain_id,
                        recipient: transfer_data.recipient,
                        recipient_chain: transfer_data.recipient_chain,
                        fee: (0, transfer_data.fee),
                    };
                    let message = spec::TokenBridgeMessage {
                        action: spec::TokenBridgeAction::Transfer,
                        payload: transfer_info.serialize(),
                    };
                    let message = message.serialize();

                    // Post the message to the wormhole contract.
                    use cbor::cbor_map;
                    ctx.emit_message(Message::Call {
                        id: REPLY_ID_OUTBOUND_TRANSFER_POST_MESSAGE,
                        reply: NotifyReply::Always,
                        method: "contracts.Call".to_string(),
                        body: cbor::cbor_map! {
                            "id" => cbor::cbor_int!(cfg.wormhole_contract.as_u64() as i64),
                            "data" => cbor::cbor_bytes!(cbor::to_vec(
                                cbor::to_value(wormhole::Request::PostMessage{message, nonce: transfer_data.nonce}),
                            )),
                            "tokens" => cbor::to_value(transfer_data.deposited_tokens), // Forward any native tokens received (fee) to the Wormhole chain.
                        },
                        max_gas: None,
                        data
                    });

                    Ok(None)
                } else {
                    Err(Error::TransferFailed)
                }
            }
            Reply::Call { id, result, .. } if id == REPLY_ID_OUTBOUND_TRANSFER_POST_MESSAGE => {
                if !result.is_success() {
                    return Err(Error::TransferFailed);
                }

                // TODO: emit event, and response.

                Ok(None)
            }
            Reply::Call {
                id, result, data, ..
            } if id == REPLY_ID_OUTBOUND_TRANSFER_OAS20 => {
                if !result.is_success() {
                    return Err(Error::TransferFailed);
                }

                let mut transfer_data: types::OutboundTransferData =
                    cbor::from_value(data.unwrap()).unwrap();

                if let oas20::Response::TokenInformation { token_information } = match result {
                    CallResult::Ok(val) => Ok(cbor::from_value(val).unwrap()),
                    _ => Err(Error::TransferFailed),
                }? {
                    // Token needs to be normalized to 8 decimals, as this is the max wormhole protocol
                    // supports.
                    let multiplier =
                        10u128.pow((max(token_information.decimals, 8u8) - 8u8) as u32);

                    // Subtract remainder due to normalization, se we don't withdraw too much.
                    transfer_data.amount = transfer_data
                        .amount
                        .checked_sub(transfer_data.amount.checked_rem(multiplier).unwrap())
                        .unwrap();
                    transfer_data.fee = transfer_data
                        .fee
                        .checked_sub(transfer_data.fee.checked_rem(multiplier).unwrap())
                        .unwrap();
                    // Withdraw amount should not be normalized.
                    let amount = transfer_data.amount;

                    // Normalize.
                    transfer_data.amount = transfer_data.amount.checked_div(multiplier).unwrap();
                    transfer_data.fee = transfer_data.fee.checked_div(multiplier).unwrap();

                    // Withdraw tokens.
                    use cbor::cbor_map;
                    ctx.emit_message(Message::Call {
                        id: REPLY_ID_OUTBOUND_TRANSFER_OAS20_WITHDRAW,
                        reply: NotifyReply::Always,
                        method: "contracts.Call".to_string(),
                        body: cbor::cbor_map! {
                            "id" => cbor::cbor_int!(transfer_data.asset.as_u64() as i64),
                            "data" => cbor::cbor_bytes!(cbor::to_vec(
                                cbor::to_value(oas20::Request::Withdraw{ from: ctx.caller_address().to_owned(), amount }),
                            )),
                            "tokens" => cbor::cbor_array![],
                        },
                        max_gas: None,
                        data: Some(cbor::to_value(
                            transfer_data,
                        )),
                    });

                    // Continues bellow.

                    Ok(None)
                } else {
                    Err(Error::TransferFailed)
                }
            }
            Reply::Call {
                id, result, data, ..
            } if id == REPLY_ID_OUTBOUND_TRANSFER_OAS20_WITHDRAW => {
                if !result.is_success() {
                    return Err(Error::TransferFailed);
                }

                // Withdraw successful. Post transfer message to wormhole contract.

                let cfg = CONFIG.get(ctx.public_store()).unwrap();

                let transfer_data: types::OutboundTransferData =
                    cbor::from_value(data.clone().unwrap()).unwrap();

                // Update locked asset state.
                let mut locked = LOCKED_ASSETS
                    .get(ctx.public_store(), transfer_data.asset.as_u64().into())
                    .unwrap();
                locked += transfer_data.amount;
                // Wormhole toke bridge spec requires that only a max of u64 token units is ever locked.
                if locked > u64::MAX as u128 {
                    return Err(Error::LockedAssetLimitExceeded);
                }
                LOCKED_ASSETS.insert(
                    ctx.public_store(),
                    transfer_data.asset.as_u64().into(),
                    locked,
                );

                let transfer_info = spec::TransferInfo {
                    amount: (0, transfer_data.amount),
                    token_address: transfer_data.asset.into(),
                    token_chain: wormhole::spec::OASIS_CHAIN_ID,
                    recipient: transfer_data.recipient,
                    recipient_chain: transfer_data.recipient_chain,
                    fee: (0, transfer_data.fee),
                };
                let message = spec::TokenBridgeMessage {
                    action: spec::TokenBridgeAction::Transfer,
                    payload: transfer_info.serialize(),
                };
                let message = message.serialize();

                // Post the message to the wormhole contract.
                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: REPLY_ID_OUTBOUND_TRANSFER_POST_MESSAGE,
                    reply: NotifyReply::Always,
                    method: "contracts.Call".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(cfg.wormhole_contract.as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(wormhole::Request::PostMessage{message, nonce: transfer_data.nonce}),
                        )),
                        "tokens" => cbor::to_value(transfer_data.deposited_tokens), // Forward any native tokens received (fee) to the Wormhole chain.
                    },
                    max_gas: None,
                    data
                });

                Ok(None)
            }
            _ => Err(Error::BadRequest),
        }
    }
}

// Create the required WASM exports required for the contract to be runnable.
sdk::create_contract!(WormholeTokenBridge);
