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

/// Verifies the signature on signed query and whether it is appropriately leashed.
///
/// See [`crate::types::SignedQueryEnvelope`] for details on the signature format.
pub(crate) fn verify<C: Context, Cfg: Config>(
    ctx: &mut C,
    query: SimulateCallQuery,
    mut signature: [u8; 65],
) -> Result<SimulateCallQuery, Error> {
    let leash = match query.leash.as_ref() {
        Some(leash) => leash,
        None => return Err(Error::InvalidSignedQuery("missing leash")),
    };

    // First, verify the signature since it's cheap compared to accessing state to verify the leash.
    if signature[64] >= 27 {
        // Some wallets generate a high recovery id, which isn't tolerated by the ecdsa crate.
        signature[64] -= 27
    }
    let sig = recoverable::Signature::try_from(signature.as_slice())
        .map_err(|_| Error::InvalidSignedQuery("invalid signature"))?;
    let signed_message = hash_call_toplevel::<Cfg>(&query);
    let signer_pk = crate::raw_tx::recover_low(&sig, &signed_message.into())
        .map_err(|_| Error::InvalidSignedQuery("signature recovery failed"))?;
    let signer_addr_digest = Keccak256::digest(&signer_pk.to_encoded_point(false).as_bytes()[1..]);
    if &signer_addr_digest[12..] != query.caller.as_ref() {
        return Err(Error::InvalidSignedQuery("signer != caller"));
    }

    // Next, verify the leash.
    let current_block = ctx.runtime_header().round;
    let mut state = ctx.runtime_state();
    let sdk_address = Cfg::map_address(query.caller.into());
    let nonce = Cfg::Accounts::get_nonce(&mut state, sdk_address).unwrap();
    if nonce > leash.nonce {
        return Err(Error::InvalidSignedQuery("stale nonce"));
    }

    let block_hashes = state::block_hashes(state);
    let base_block_hash = match block_hashes.get::<_, Hash>(&leash.block_number.to_be_bytes()) {
        Some(hash) => hash,
        None => return Err(Error::InvalidSignedQuery("base block not found")),
    };
    if base_block_hash.as_ref() != leash.block_hash.as_ref() {
        return Err(Error::InvalidSignedQuery("unexpected base block"));
    }
    let block_delta = current_block
        .checked_sub(leash.block_number)
        .unwrap_or_else(|| leash.block_number - current_block);
    if block_delta > leash.block_range {
        return Err(Error::InvalidSignedQuery("current block out of range"));
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
            ",bytes32 blockHash",
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
        ",uint64 gasLimit",
        ",uint256 gasPrice",
        ",uint256 value",
        ",bytes data",
        ",Leash leash",
        ")",
        leash_type_str!()
    );
    hash_encoded(&[
        encode_bytes(CALL_TYPE_STR),
        Token::Address(query.caller.0.into()),
        Token::Address(query.address.0.into()),
        Token::Uint(query.gas_limit.into()),
        Token::Uint(ethabi::ethereum_types::U256(query.gas_price.0)),
        Token::Uint(ethabi::ethereum_types::U256(query.value.0)),
        encode_bytes(&query.data),
        Token::Uint(hash_leash(query.leash.as_ref().unwrap() /* checked in verify */).into()),
    ])
}

fn hash_leash(leash: &Leash) -> [u8; 32] {
    hash_encoded(&[
        encode_bytes(leash_type_str!()),
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
            encode_bytes(DOMAIN_TYPE_STR),
            encode_bytes("oasis-runtime-sdk/evm: signed query"),
            encode_bytes("1.0.0"),
            Token::Uint(Cfg::CHAIN_ID.into()),
        ])
    })
}

fn encode_bytes(s: impl AsRef<[u8]>) -> Token {
    Token::FixedBytes(Keccak256::digest(s.as_ref()).to_vec())
}

fn hash_encoded(tokens: &[Token]) -> [u8; 32] {
    Keccak256::digest(&ethabi::encode(tokens)).into()
}

#[cfg(test)]
mod test {
    use super::*;

    use oasis_runtime_sdk::testing::mock;

    use crate::{
        test::{ConfidentialEVMConfig as C10lCfg, EVMConfig as Cfg},
        types::SignedQueryEnvelope,
        Module as EVMModule,
    };

    /// This was generated using the `@oasislabs/sapphire-paratime` JS lib.
    const SIGNED_QUERY: &str =
"a2657175657279a764646174614401020304656c65617368a4656e6f6e63651903e76a626c6f636b5f686173685820c92b675c7013e33aa88feaae520eb0ede155e7cacb3c4587e0923cba9953f8bb6b626c6f636b5f72616e6765036c626c6f636b5f6e756d626572182a6576616c75655820000000000000000000000000000000000000000000000000000000000000002a6663616c6c65725411e244400cf165ade687077984f09c3a037b868f676164647265737354b5ed90452aac09f294a0be877cbf2dc4d55e096f696761735f6c696d69740a696761735f70726963655820000000000000000000000000000000000000000000000000000000000000007b697369676e6174757265584148bca100e84d13a80b131c62b9b87caf07e4da6542a9e1ea16d8042ba08cc1e31f10ae924d8c137882204e9217423194014ce04fa2130c14f27b148858733c7b1c";

    fn make_signed_query() -> SignedQueryEnvelope {
        let signed_query = hex::decode(SIGNED_QUERY).unwrap();
        cbor::from_slice(&signed_query).unwrap()
    }

    fn setup_context<C: Context>(ctx: &mut C, query: &SimulateCallQuery) {
        setup_nonce(ctx, query);
        setup_block(ctx, query);
    }

    fn setup_nonce<C: Context>(ctx: &mut C, query: &SimulateCallQuery) {
        let leash = query.leash.as_ref().unwrap();
        let mut state = ctx.runtime_state();
        let sdk_address = C10lCfg::map_address(query.caller.into());
        <C10lCfg as Config>::Accounts::set_nonce(&mut state, sdk_address, leash.nonce);
    }

    fn setup_stale_nonce<C: Context>(ctx: &mut C, query: &SimulateCallQuery) {
        let leash = query.leash.as_ref().unwrap();
        let mut state = ctx.runtime_state();
        let sdk_address = C10lCfg::map_address(query.caller.into());
        <C10lCfg as Config>::Accounts::set_nonce(&mut state, sdk_address, leash.nonce + 1);
    }

    fn setup_block<C: Context>(ctx: &mut C, query: &SimulateCallQuery) {
        let leash = query.leash.as_ref().unwrap();
        let mut block_hashes = state::block_hashes(ctx.runtime_state());
        block_hashes.insert::<_, Hash>(
            &leash.block_number.to_be_bytes(),
            leash.block_hash.as_ref().into(),
        );
    }

    #[test]
    fn test_verify_ok() {
        let SignedQueryEnvelope { query, signature } = make_signed_query();
        let leash = query.leash.as_ref().unwrap();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = leash.block_number;
        let mut ctx = mock.create_ctx();

        setup_context(&mut ctx, &query);

        verify::<_, C10lCfg>(&mut ctx, query, signature).unwrap();
    }

    #[test]
    fn test_verify_bad_signature() {
        let SignedQueryEnvelope {
            query,
            mut signature,
        } = make_signed_query();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = query.leash.as_ref().unwrap().block_number;
        let mut ctx = mock.create_ctx();

        setup_context(&mut ctx, &query);

        signature[0] = signature[0].wrapping_add(1);
        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, signature).unwrap_err(),
            Error::InvalidSignedQuery("signer != caller")
        ));
    }

    #[test]
    fn test_verify_bad_nonce() {
        let SignedQueryEnvelope { query, signature } = make_signed_query();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = query.leash.as_ref().unwrap().block_number;
        let mut ctx = mock.create_ctx();

        setup_block(&mut ctx, &query);
        setup_stale_nonce(&mut ctx, &query);

        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, signature).unwrap_err(),
            Error::InvalidSignedQuery("stale nonce")
        ));
    }

    #[test]
    fn test_verify_bad_base_block() {
        let SignedQueryEnvelope { query, signature } = make_signed_query();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = query.leash.as_ref().unwrap().block_number;
        let mut ctx = mock.create_ctx();

        setup_nonce(&mut ctx, &query);

        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, signature).unwrap_err(),
            Error::InvalidSignedQuery("base block not found")
        ));
    }

    #[test]
    fn test_verify_bad_range() {
        let SignedQueryEnvelope { query, signature } = make_signed_query();

        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx();

        setup_context(&mut ctx, &query);

        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, signature).unwrap_err(),
            Error::InvalidSignedQuery("current block out of range")
        ));
    }

    #[test]
    fn test_decode_simulate_call_query() {
        let signed_query_bytes = hex::decode(SIGNED_QUERY).unwrap();
        let unsigned_body = make_signed_query().query;
        let signed_body = SimulateCallQuery {
            data: signed_query_bytes,
            ..Default::default()
        };

        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx();

        let mut c10l_mock = mock::Mock::default();
        c10l_mock.runtime_header.round = unsigned_body.leash.as_ref().unwrap().block_number;
        let mut c10l_ctx = c10l_mock.create_ctx();
        setup_context(&mut c10l_ctx, &unsigned_body);

        let mut non_c10l_decode = |body: &SimulateCallQuery| {
            EVMModule::<Cfg>::decode_simulate_call_query(&mut ctx, body.clone())
        };
        let mut c10l_decode = |body: &SimulateCallQuery| {
            EVMModule::<C10lCfg>::decode_simulate_call_query(&mut c10l_ctx, body.clone())
        };

        assert_eq!(non_c10l_decode(&unsigned_body).unwrap(), unsigned_body);
        assert_eq!(non_c10l_decode(&signed_body).unwrap(), signed_body);

        assert!(EVMModule::<C10lCfg>::decode_simulate_call_query(
            &mut mock::Mock::default().create_ctx(),
            signed_body.clone()
        )
        .is_err()); // Check that errors are propagated (in this case leash invalidity).
        assert_eq!(c10l_decode(&signed_body).unwrap(), unsigned_body);
        assert_eq!(
            c10l_decode(&unsigned_body).unwrap(),
            SimulateCallQuery {
                caller: Default::default(),
                ..unsigned_body
            }
        );
    }
}
