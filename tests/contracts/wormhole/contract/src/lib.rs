//! Oasis Wormhole contract.
#![feature(wasm_abi)]

extern crate alloc;

use sha3::{Digest, Keccak256};

use oasis_contract_sdk::{
    self as sdk,
    env::{Crypto, Env},
    types::{
        env::{QueryRequest, QueryResponse},
        token,
    },
};
use oasis_contract_sdk_storage::{
    cell::Cell,
    map::{Int, Map},
};
use oasis_contract_sdk_types::address::Address;

use oasis_wormhole_types::{self as types, spec, Error, Event, Request, Response};

#[cfg(test)]
mod test;

/// The contract type.
pub struct Wormhole;

/// Storage cell for the config.
const CONFIG: Cell<types::Config> = Cell::new(b"config");
/// Storage map for the guardian set state.
const GS_STATE: Map<Int<u32>, spec::GuardianSet> = Map::new(b"gs_state");
/// Storage map of the post message address sequences.
const SEQUENCE_NUMS: Map<Address, u64> = Map::new(b"sequence_nums");
/// Storage map of previously submitted VAAs.
const VAAS: Map<&[u8], bool> = Map::new(b"vaas");

/// Wormhole module name of the contract.
const MODULE_NAME: &str = "Core";

impl Wormhole {
    fn gov_vaa_update_guardian_set<C: sdk::Context>(ctx: &mut C, data: &[u8]) -> Result<(), Error> {
        // Query block timestamp.
        let timestamp = match ctx.env().query(QueryRequest::BlockInfo) {
            QueryResponse::BlockInfo { timestamp, .. } => Ok(timestamp),
            _ => Err(Error::QueryFailed),
        }?;

        let mut cfg = CONFIG.get(ctx.public_store()).unwrap();
        let expiry = cfg.guardian_set_expiry;

        let spec::GuardianSetUpgrade {
            new_guardian_set_index,
            new_guardian_set,
        } = spec::GuardianSetUpgrade::deserialize(data)?;

        if new_guardian_set_index != cfg.guardian_set_index + 1 {
            return Err(Error::InvalidGuardianSetUpgradeIndex);
        }

        let old_guardian_set_index = cfg.guardian_set_index;
        cfg.guardian_set_index = new_guardian_set_index;

        GS_STATE.insert(
            ctx.public_store(),
            cfg.guardian_set_index.into(),
            new_guardian_set,
        );
        CONFIG.set(ctx.public_store(), cfg);

        // Update previous GS expiration time to: timestamp+cfg.guardian_set_expiry;
        let mut old_guardian_set = GS_STATE
            .get(ctx.public_store(), old_guardian_set_index.into())
            .unwrap();
        old_guardian_set.expiration_time = timestamp + expiry;
        GS_STATE.insert(
            ctx.public_store(),
            old_guardian_set_index.into(),
            old_guardian_set,
        );

        ctx.emit_event(Event::GuardianSetUpdate {
            index: new_guardian_set_index,
        });

        Ok(())
    }

    fn gov_vaa_set_fee<C: sdk::Context>(ctx: &mut C, data: &[u8]) -> Result<(), Error> {
        let set_fee_msg = spec::SetFee::deserialize(data)?;

        let mut cfg = CONFIG.get(ctx.public_store()).unwrap();
        cfg.fee = set_fee_msg.fee.clone();
        CONFIG.set(ctx.public_store(), cfg);

        ctx.emit_event(Event::FeeUpdate {
            fee: set_fee_msg.fee,
        });

        Ok(())
    }

    fn gov_vaa_transfer_fee<C: sdk::Context>(_ctx: &mut C, data: &[u8]) -> Result<(), Error> {
        let _transfer_msg = spec::TransferFee::deserialize(data)?;

        // TODO: Emit accounts transfer message.

        Ok(())
    }
}

impl Wormhole {
    fn parse_and_verify_vaa<C: sdk::Context>(
        ctx: &mut C,
        data: &[u8],
        block_time: u64,
    ) -> Result<spec::ParsedVAA, Error> {
        let vaa = spec::ParsedVAA::deserialize(data).map_err(Error::InvalidVAA)?;

        if vaa.version != 1 {
            return Err(Error::InvalidVAAVersion);
        }

        // Check VAAS state if VAA with this hash was already accepted.
        if VAAS.get(ctx.public_store(), &vaa.hash).unwrap_or_default() {
            return Err(Error::VAAAlreadyExecuted);
        }

        let guardian_set = GS_STATE
            .get(ctx.public_store(), vaa.guardian_set_index.into())
            .ok_or(Error::VAAInvalidGuardianSetIndex)?;

        if guardian_set.expiration_time != 0 && guardian_set.expiration_time < block_time {
            return Err(Error::VAAGuardianSetExpired);
        }

        if (vaa.len_signers as usize) < guardian_set.quorum() {
            return Err(Error::VAANoQuorum);
        }

        if (vaa.len_signers as usize) > guardian_set.addresses.len() {
            return Err(Error::VAATooManySignatures);
        }

        // Verify signatures.
        for (index, signature) in vaa.signatures.iter().enumerate() {
            let mut input = [0u8; 97];
            input[0..32].copy_from_slice(&vaa.hash);
            input[32..].copy_from_slice(signature);
            let key = ctx.env().ecdsa_recover(&input);

            // Covert to an ethereum address.
            let mut hasher = Keccak256::new();
            hasher.update(&key[1..]);
            let address = hasher.finalize();
            let address = spec::GuardianAddress::from_bytes(&address[32 - 20..])
                .map_err(|_| Error::VAAGuardianSignatureError)?;

            // Ensure verify address matches the guardian set address at index.
            if address != guardian_set.addresses[index] {
                return Err(Error::VAAGuardianSignatureError);
            }
        }

        Ok(vaa)
    }

    fn post_message<C: sdk::Context>(
        ctx: &mut C,
        message: Vec<u8>,
        nonce: u32,
    ) -> Result<(), Error> {
        let cfg = CONFIG.get(ctx.public_store()).unwrap();

        // Ensure sufficient fee was deposited.
        let mut sufficient_fee = false;
        for tokens in ctx.deposited_tokens() {
            if tokens.denomination() == cfg.fee.denomination()
                && tokens.amount() >= cfg.fee.amount()
            {
                sufficient_fee = true;
                break;
            }
        }
        if !sufficient_fee {
            return Err(Error::InsufficientFeePaid);
        }

        // Query block timestamp.
        let timestamp = match ctx.env().query(QueryRequest::BlockInfo) {
            QueryResponse::BlockInfo { timestamp, .. } => Ok(timestamp),
            _ => Err(Error::QueryFailed),
        }?;

        // Bump senders sequence number.
        let sender = ctx.caller_address().to_owned();
        let sequence = SEQUENCE_NUMS
            .get(ctx.public_store(), sender)
            .unwrap_or_default();
        SEQUENCE_NUMS.insert(ctx.public_store(), sender, sequence + 1);

        // Emit event.
        ctx.emit_event(Event::PostMessage {
            message,
            nonce,
            sender: sender.into(),
            chain_id: spec::OASIS_CHAIN_ID,
            sequence,
            block_time: timestamp,
        });

        Ok(())
    }

    fn submit_vaa<C: sdk::Context>(ctx: &mut C, data: &[u8]) -> Result<(), Error> {
        let cfg = CONFIG.get(ctx.public_store()).unwrap();

        // Query timestamp.
        let timestamp = match ctx.env().query(QueryRequest::BlockInfo) {
            QueryResponse::BlockInfo { timestamp, .. } => Ok(timestamp),
            _ => Err(Error::QueryFailed),
        }?;

        let vaa = Self::parse_and_verify_vaa(ctx, data, timestamp)?;
        // Store VAA as seen.
        VAAS.insert(ctx.public_store(), &vaa.hash, true);

        // Wormhole contract handle only governance packets.
        if cfg.governance_chain != vaa.emitter_chain
            || cfg.governance_address != vaa.emitter_address
        {
            return Err(Error::InvalidVAAAction);
        }

        // Governance VAAs must be signed by latest guardian set.
        if cfg.guardian_set_index != vaa.guardian_set_index {
            return Err(Error::InvalidGuardianSetForGovernance);
        }

        let gov_packet = spec::GovernancePacket::deserialize(&vaa.payload)?;

        if gov_packet.module != MODULE_NAME {
            return Err(Error::InvalidVAAModule);
        }

        if gov_packet.chain != 0 && gov_packet.chain != spec::OASIS_CHAIN_ID {
            return Err(Error::InvalidVAAChainId);
        }

        // Handle the governance action.
        match gov_packet.action {
            spec::GovernanceAction::UpdateGuardianSet => {
                Self::gov_vaa_update_guardian_set(ctx, &gov_packet.payload)
            }
            spec::GovernanceAction::SetFee => Self::gov_vaa_set_fee(ctx, &gov_packet.payload),
            spec::GovernanceAction::TransferFee => {
                Self::gov_vaa_transfer_fee(ctx, &gov_packet.payload)
            }
        }
    }
}

// Implementation of the sdk::Contract trait is required in order for the type to be a contract.
impl sdk::Contract for Wormhole {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    fn instantiate<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<(), Error> {
        match request {
            Request::Instantiate { params } => {
                // Initialize config.
                let cfg = types::Config {
                    guardian_set_index: params.initial_guardian_set_index,
                    guardian_set_expiry: params.guardian_set_expiry,
                    governance_chain: params.governance_chain,
                    governance_address: params.governance_address,
                    fee: token::BaseUnits::new(params.fee, token::Denomination::NATIVE),
                };

                if params.initial_guardian_set.addresses.is_empty() {
                    return Err(Error::EmptyGuardianSet);
                }

                GS_STATE.insert(
                    ctx.public_store(),
                    cfg.guardian_set_index.into(),
                    params.initial_guardian_set,
                );
                CONFIG.set(ctx.public_store(), cfg);

                Ok(())
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        match request {
            Request::PostMessage { message, nonce } => {
                Self::post_message(ctx, message, nonce)?;
                Ok(Response::Empty)
            }
            Request::SubmitVAA { vaa } => {
                Self::submit_vaa(ctx, &vaa)?;
                Ok(Response::Empty)
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn query<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        match request {
            Request::GuardianSetInfo => {
                // Returns the current guardian set index and addresses.
                let cfg = CONFIG.get(ctx.public_store()).unwrap();
                let guardian_set = GS_STATE
                    .get(ctx.public_store(), cfg.guardian_set_index.into())
                    .unwrap();

                Ok(Response::GuardianSetInfo { guardian_set })
            }
            Request::VerifyVAA { vaa, block_time } => {
                let vaa = Self::parse_and_verify_vaa(ctx, &vaa, block_time)?;
                Ok(Response::VerifiedVAA { vaa })
            }
            Request::GetConfig => {
                let cfg = CONFIG.get(ctx.public_store()).unwrap();
                Ok(Response::GetConfig { config: cfg })
            }
            _ => Err(Error::BadRequest),
        }
    }
}

// Create the required WASM exports required for the contract to be runnable.
sdk::create_contract!(Wormhole);
