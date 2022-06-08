use std::convert::TryFrom as _;

use ethabi::Token;
use k256::{ecdsa::recoverable, elliptic_curve::sec1::ToEncodedPoint as _};
use once_cell::sync::OnceCell;
use sha3::{Digest as _, Keccak256};

use oasis_runtime_sdk::{
    context::Context, core::common::crypto::hash::Hash, modules::accounts::API as _,
};

use crate::{
    state,
    types::{Leash, SimulateCallQuery},
    Config, Error,
};

pub(super) fn verify<C: Context, Cfg: Config>(
    ctx: &mut C,
    query: SimulateCallQuery,
    signature: &[u8; 65],
) -> Result<SimulateCallQuery, Error> {
    let leash = match query.leash.as_ref() {
        Some(leash) => leash,
        None => return Err(Error::InvalidSignedQuery("missing leash")),
    };

    // First, verify the signature since it's cheap compared to accessing state to verify the leash.
    let invaid_signature = || Error::InvalidSignedQuery("invalid signature");
    let sig =
        recoverable::Signature::try_from(signature.as_slice()).map_err(|_| invaid_signature())?;
    if sig.s().is_high().into() {
        return Err(invaid_signature());
    }
    let signed_message = hash_call_toplevel::<Cfg>(&query);
    let signer_key = sig
        .recover_verify_key_from_digest_bytes(&signed_message.into())
        .map_err(|_| invaid_signature())?;
    let signer_addr_digest = Keccak256::digest(&signer_key.to_encoded_point(false).as_bytes()[1..]);
    if &signer_addr_digest[12..] != query.caller.as_ref() {
        return Err(invaid_signature());
    }

    // Next, verify the leash.
    let mut state = ctx.runtime_state();
    let sdk_address = Cfg::map_address(query.caller.into());
    let nonce = Cfg::Accounts::get_nonce(&mut state, sdk_address).unwrap();
    if nonce < leash.nonce {
        return Err(Error::InvalidSignedQuery("stale nonce"));
    }

    let block_hashes = state::block_hashes(state);
    if let Some(hash) = block_hashes.get::<_, Hash>(&leash.block_number.to_be_bytes()) {
        if hash.as_ref() != leash.block_hash.as_ref() {
            return Err(Error::InvalidSignedQuery("unexpected base block"));
        }
    } else {
        return Err(Error::InvalidSignedQuery("base block out of range"));
    }

    Ok(query)
}

macro_rules! leash_type_str {
    () => {
        concat!(
            "Leash",
            "(",
            "uint64 nonce",
            ",uint64 blockNumber",
            ",uint256 blockHash",
            ",uint64 blockRange",
            ")",
        )
    };
}

fn hash_call_toplevel<Cfg: Config>(query: &SimulateCallQuery) -> [u8; 32] {
    let call_struct_hash = hash_call(query);
    let domain_separator = hash_domain::<Cfg>();
    let mut encoded_call = [0u8; 66];
    encoded_call[0..2].copy_from_slice(b"\x19\x01");
    encoded_call[2..34].copy_from_slice(domain_separator);
    encoded_call[34..].copy_from_slice(&call_struct_hash);
    Keccak256::digest(&encoded_call).into()
}

fn hash_call(query: &SimulateCallQuery) -> [u8; 32] {
    const CALL_TYPE_STR: &str = concat!(
        "Call",
        "(",
        "address from",
        ",address to",
        ",uint256 value",
        ",uint256 gasPrice",
        ",uint64 gasLimit",
        ",bytes data",
        ",Leash leash",
        ")",
        leash_type_str!()
    );
    hash_encoded(&[
        encode_str(CALL_TYPE_STR),
        Token::Address(query.caller.0.into()),
        Token::Address(query.address.0.into()),
        Token::Uint(ethabi::ethereum_types::U256(query.value.0)),
        Token::Uint(ethabi::ethereum_types::U256(query.gas_price.0)),
        Token::Uint(query.gas_limit.into()),
        Token::Bytes(query.data.clone()),
        Token::Uint(hash_leash(query.leash.as_ref().unwrap() /* checked above */).into()),
    ])
}

fn hash_leash(leash: &Leash) -> [u8; 32] {
    hash_encoded(&[
        encode_str(leash_type_str!()),
        Token::Uint(leash.nonce.into()),
        Token::Uint(leash.block_number.into()),
        Token::Uint(leash.block_hash.0.into()),
        Token::Uint(leash.block_range.into()),
    ])
}

fn hash_domain<Cfg: Config>() -> &'static [u8; 32] {
    static DOMAIN_SEPARATOR: OnceCell<[u8; 32]> = OnceCell::new(); // Not `Lazy` because of generic.
    DOMAIN_SEPARATOR.get_or_init(|| {
        const DOMAIN_TYPE_STR: &str = "EIP712Domain(string name,string version,uint256 chainId)";
        hash_encoded(&[
            encode_str(DOMAIN_TYPE_STR),
            encode_str("Sapphire Paratime"),
            encode_str("1.0.0"),
            Token::Uint(Cfg::CHAIN_ID.into()),
        ])
    })
}

fn encode_str(s: &str) -> Token {
    Token::FixedBytes(Keccak256::digest(s.as_bytes()).to_vec())
}

fn hash_encoded(tokens: &[Token]) -> [u8; 32] {
    Keccak256::digest(&ethabi::encode(tokens)).into()
}
