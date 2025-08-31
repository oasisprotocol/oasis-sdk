use std::convert::TryFrom as _;

use blake3::hash;
use ethabi::Token;
use once_cell::sync::OnceCell;
use sha3::{Digest as _, Keccak256};

use oasis_runtime_sdk::{
    context::Context, core::common::crypto::hash::Hash, modules::accounts::API as _,
    state::CurrentState,
};

use crate::{
    state,
    types::{Leash, SimulateCallQuery},
    Config, Error, Runtime,
};

// Convert U256 representation from [u64; 4] to [u8; 32] (the u64s seem to be little endian)
fn u64x4_to_u8x32(arr: [u64; 4]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, v) in arr.iter().rev().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&v.to_be_bytes());
    }
    out
}

// Pad a [u8; 20] to [u8; 32] (left pad with zeros)
fn pad_address_to_u8x32(addr: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(addr);
    out
}

/// Verifies the signature on signed query and whether it is appropriately leashed.
///
/// See [`crate::types::SignedSimulateCallEnvelope`] for details on the signature format.
pub(crate) fn verify<C: Context, Cfg: Config>(
    ctx: &C,
    query: SimulateCallQuery,
    leash: Leash,
    mut signature: [u8; 65],
) -> Result<SimulateCallQuery, Error> {
    // First, verify the signature since it's cheap compared to accessing state to verify the leash.
    if signature[64] >= 27 {
        // Some wallets generate a high recovery id, which isn't tolerated by the ecdsa crate.
        signature[64] -= 27
    }
    let sig = k256::ecdsa::Signature::try_from(&signature[..64])
        .map_err(|_| Error::InvalidSignedSimulateCall("invalid signature"))?;
    let sig_recid = k256::ecdsa::RecoveryId::from_byte(signature[64])
        .ok_or(Error::InvalidSignedSimulateCall("invalid signature"))?;
    let signed_message = hash_call_toplevel::<Cfg>(&query, &leash);
    let signer_pk = crate::raw_tx::recover_low(&sig, sig_recid, &signed_message.into())
        .map_err(|_| Error::InvalidSignedSimulateCall("signature recovery failed"))?;
    let signer_addr_digest = Keccak256::digest(&signer_pk.to_encoded_point(false).as_bytes()[1..]);
    if &signer_addr_digest[12..] != query.caller.as_ref() {
        return Err(Error::InvalidSignedSimulateCall("signer != caller"));
    }

    // Next, verify the leash.
    let current_block = ctx.runtime_header().round;
    let sdk_address = Cfg::map_address(query.caller.into());
    let nonce = <C::Runtime as Runtime>::Accounts::get_nonce(sdk_address).unwrap();
    if nonce > leash.nonce {
        return Err(Error::InvalidSignedSimulateCall("stale nonce"));
    }

    let base_block_hash = CurrentState::with_store(|store| {
        let block_hashes = state::block_hashes(store);
        match block_hashes.get::<_, Hash>(&leash.block_number.to_be_bytes()) {
            Some(hash) => Ok(hash),
            None => Err(Error::InvalidSignedSimulateCall("base block not found")),
        }
    })?;
    if base_block_hash.as_ref() != leash.block_hash.as_ref() {
        return Err(Error::InvalidSignedSimulateCall("unexpected base block"));
    }

    #[allow(clippy::unnecessary_lazy_evaluations)]
    let block_delta = current_block
        .checked_sub(leash.block_number)
        .unwrap_or_else(|| leash.block_number - current_block);
    if block_delta > leash.block_range {
        return Err(Error::InvalidSignedSimulateCall(
            "current block out of range",
        ));
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

fn hash_call_toplevel<Cfg: Config>(query: &SimulateCallQuery, leash: &Leash) -> [u8; 32] {
    let call_struct_hash = hash_call(query, leash);
    let domain_separator = hash_domain::<Cfg>();
    let mut encoded_call = [0u8; 66];
    encoded_call[0..2].copy_from_slice(b"\x19\x01");
    encoded_call[2..34].copy_from_slice(domain_separator);
    encoded_call[34..].copy_from_slice(&call_struct_hash);
    Keccak256::digest(encoded_call).into()
}

fn hash_call(query: &SimulateCallQuery, leash: &Leash) -> [u8; 32] {
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
    let encoded = solabi::encode_packed(&(
        hash_bytes(CALL_TYPE_STR),
        solabi::U256::from_be_bytes(pad_address_to_u8x32(&query.caller.0)),
        solabi::U256::from_be_bytes(pad_address_to_u8x32(&query.address.unwrap_or_default().0)),
        solabi::U256::new(query.gas_limit.into()),
        solabi::U256::from_be_bytes(u64x4_to_u8x32(query.gas_price.0)),
        solabi::U256::from_be_bytes(u64x4_to_u8x32(query.value.0)),
        hash_bytes(&query.data),
        solabi::U256::from_be_bytes(hash_leash(&leash)),
    ));
    Keccak256::digest(encoded).into()
}

fn hash_call_old(query: &SimulateCallQuery, leash: &Leash) -> [u8; 32] {
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
        Token::Address(query.address.unwrap_or_default().0.into()),
        Token::Uint(query.gas_limit.into()),
        Token::Uint(ethabi::ethereum_types::U256(query.gas_price.0)),
        Token::Uint(ethabi::ethereum_types::U256(query.value.0)),
        encode_bytes(&query.data),
        Token::Uint(hash_leash(leash).into()),
    ])
}

fn hash_leash(leash: &Leash) -> [u8; 32] {
    let encoded = solabi::encode_packed(&(
        hash_bytes(leash_type_str!()),
        solabi::U256::new(leash.nonce.into()),
        solabi::U256::new(leash.block_number.into()),
        solabi::U256::from_be_bytes(leash.block_hash.0.into()),
        solabi::U256::new(leash.block_range.into()),
    ));
    Keccak256::digest(encoded).into()
}

fn hash_leash_old(leash: &Leash) -> [u8; 32] {
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
        let encoded = solabi::encode_packed(&(
            hash_bytes(DOMAIN_TYPE_STR),
            hash_bytes("oasis-runtime-sdk/evm: signed query"),
            hash_bytes("1.0.0"),
            solabi::U256::new(Cfg::CHAIN_ID.into()),
        ));
        Keccak256::digest(encoded).into()
    })
}

fn encode_bytes(s: impl AsRef<[u8]>) -> Token {
    Token::FixedBytes(Keccak256::digest(s.as_ref()).to_vec())
}

fn hash_bytes(s: impl AsRef<[u8]>) -> Vec<u8> {
    Keccak256::digest(s.as_ref()).to_vec()
}

fn hash_encoded(tokens: &[Token]) -> [u8; 32] {
    Keccak256::digest(ethabi::encode(tokens)).into()
}

#[cfg(test)]
mod test {
    use super::*;

    use oasis_runtime_sdk::{modules::accounts, testing::mock};

    use crate::{
        test::{ConfidentialEVMConfig as C10lCfg, EVMConfig as Cfg},
        types::{SignedCallDataPack, SimulateCallQuery, H160, H256},
        Module as EVMModule,
    };

    type Accounts = accounts::Module;

    #[test]
    fn test_u64x4_to_u8x32() {
        let input = [1u64, 2, 3, 4];
        let expected = [
            0,0,0,0,0,0,0,4,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,1
        ];
        assert_eq!(u64x4_to_u8x32(input), expected);
    }

    #[test]
    fn test_hash_domain_refactor() {
        const DOMAIN_TYPE_STR: &str = "EIP712Domain(string name,string version,uint256 chainId)";
 
        let old_domain_encode = ethabi::encode(&[
            encode_bytes(DOMAIN_TYPE_STR),
            encode_bytes("oasis-runtime-sdk/evm: signed query"),
            encode_bytes("1.0.0"),
            Token::Uint(Cfg::CHAIN_ID.into()),
        ]);

        let new_domain_encode = solabi::encode_packed(&(
            hash_bytes(DOMAIN_TYPE_STR),
            hash_bytes("oasis-runtime-sdk/evm: signed query"),
            hash_bytes("1.0.0"),
            solabi::U256::new(Cfg::CHAIN_ID.into()),
        ));

        assert_eq!(old_domain_encode, new_domain_encode)        
    }

    #[test]
    fn test_hash_leash_refactor() {
        let leash: Leash = Leash {
            nonce: 1,
            block_number: 2,
            block_hash: H256::zero(),
            block_range: 3,
        };

        let old_leash_hash = hash_leash_old(&leash);
        let new_leash_hash = hash_leash(&leash);
        assert_eq!(old_leash_hash, new_leash_hash);
    }

    #[test]
    fn test_hash_call_refactor() {
        // Prepare a sample query and leash
        let leash = Leash {
            nonce: 1,
            block_number: 2,
            block_hash: H256::zero(),
            block_range: 3,
        };
        let query = SimulateCallQuery {
            gas_price: 123u64.into(),
            gas_limit: 10,
            caller: "0x11e244400Cf165ade687077984F09c3A037b868F".parse().unwrap(),
            address: Some("0xb5ed90452AAC09f294a0BE877CBf2Dc4D55e096f".parse().unwrap()),
            value: 42u64.into(),
            data: vec![1, 2, 3, 4],
        };
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

        print!("Query gas price {:?}", query.gas_price.0);

        // New
        let encoded_new = solabi::encode_packed(&(
            hash_bytes(CALL_TYPE_STR),
            solabi::U256::from_be_bytes(pad_address_to_u8x32(&query.caller.0)),
            solabi::U256::from_be_bytes(pad_address_to_u8x32(&query.address.unwrap_or_default().0)),
            solabi::U256::new(query.gas_limit.into()),
            solabi::U256::from_be_bytes(u64x4_to_u8x32(query.gas_price.0)),
            solabi::U256::from_be_bytes(u64x4_to_u8x32(query.value.0)),
            hash_bytes(&query.data),
            solabi::U256::from_be_bytes(hash_leash(&leash)),
        ));

        // Old
        let encoded_old = ethabi::encode(&[
            encode_bytes(CALL_TYPE_STR),
            Token::Address(query.caller.0.into()),
            Token::Address(query.address.unwrap_or_default().0.into()),
            Token::Uint(query.gas_limit.into()),
            Token::Uint(ethabi::ethereum_types::U256(query.gas_price.0)),
            Token::Uint(ethabi::ethereum_types::U256(query.value.0)),
            encode_bytes(&query.data),
            Token::Uint(hash_leash(&leash).into()),
        ]);

        assert_eq!(encoded_new, encoded_old);
    }


    /// This was generated using the `@oasislabs/sapphire-paratime` JS lib.
    const SIGNED_CALL_DATA_PACK: &str =
"a36464617461a164626f64794401020304656c65617368a4656e6f6e63651903e76a626c6f636b5f686173685820c92b675c7013e33aa88feaae520eb0ede155e7cacb3c4587e0923cba9953f8bb6b626c6f636b5f72616e6765036c626c6f636b5f6e756d626572182a697369676e6174757265584148bca100e84d13a80b131c62b9b87caf07e4da6542a9e1ea16d8042ba08cc1e31f10ae924d8c137882204e9217423194014ce04fa2130c14f27b148858733c7b1c";

    fn make_signed_call() -> (SimulateCallQuery, SignedCallDataPack) {
        let data_pack: SignedCallDataPack =
            cbor::from_slice(&hex::decode(SIGNED_CALL_DATA_PACK).unwrap()).unwrap();
        (
            SimulateCallQuery {
                gas_price: 123u64.into(),
                gas_limit: 10,
                caller: "0x11e244400Cf165ade687077984F09c3A037b868F"
                    .parse()
                    .unwrap(),
                address: Some(
                    "0xb5ed90452AAC09f294a0BE877CBf2Dc4D55e096f"
                        .parse()
                        .unwrap(),
                ),
                value: 42u64.into(),
                data: cbor::from_value(data_pack.data.body.clone()).unwrap(),
            },
            data_pack,
        )
    }

    fn setup_nonce(caller: &H160, leash: &Leash) {
        let sdk_address = C10lCfg::map_address((*caller).into());
        Accounts::set_nonce(sdk_address, leash.nonce);
    }

    fn setup_stale_nonce(caller: &H160, leash: &Leash) {
        let sdk_address = C10lCfg::map_address((*caller).into());
        Accounts::set_nonce(sdk_address, leash.nonce + 1);
    }

    fn setup_block(leash: &Leash) {
        CurrentState::with_store(|store| {
            let mut block_hashes = state::block_hashes(store);
            block_hashes.insert::<_, Hash>(
                &leash.block_number.to_be_bytes(),
                leash.block_hash.as_ref().into(),
            );
        });
    }

    #[test]
    fn test_verify_ok() {
        let (query, data_pack) = make_signed_call();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = data_pack.leash.block_number;
        let mut ctx = mock.create_ctx();

        setup_nonce(&query.caller, &data_pack.leash);
        setup_block(&data_pack.leash);

        verify::<_, C10lCfg>(&mut ctx, query, data_pack.leash, data_pack.signature).unwrap();
    }

    #[test]
    fn test_verify_bad_signature() {
        let (query, mut data_pack) = make_signed_call();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = data_pack.leash.block_number;
        let mut ctx = mock.create_ctx();

        setup_nonce(&query.caller, &data_pack.leash);
        setup_block(&data_pack.leash);

        data_pack.signature[0] ^= 1;
        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, data_pack.leash, data_pack.signature)
                .unwrap_err(),
            Error::InvalidSignedSimulateCall("signer != caller")
        ));
    }

    #[test]
    fn test_verify_bad_nonce() {
        let (query, data_pack) = make_signed_call();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = data_pack.leash.block_number;
        let mut ctx = mock.create_ctx();

        setup_stale_nonce(&query.caller, &data_pack.leash);
        setup_block(&data_pack.leash);

        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, data_pack.leash, data_pack.signature)
                .unwrap_err(),
            Error::InvalidSignedSimulateCall("stale nonce")
        ));
    }

    #[test]
    fn test_verify_bad_base_block() {
        let (query, data_pack) = make_signed_call();

        let mut mock = mock::Mock::default();
        mock.runtime_header.round = data_pack.leash.block_number;
        let mut ctx = mock.create_ctx();

        setup_nonce(&query.caller, &data_pack.leash);

        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, data_pack.leash, data_pack.signature)
                .unwrap_err(),
            Error::InvalidSignedSimulateCall("base block not found")
        ));
    }

    #[test]
    fn test_verify_bad_range() {
        let (query, data_pack) = make_signed_call();

        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx();

        setup_nonce(&query.caller, &data_pack.leash);
        setup_block(&data_pack.leash);

        assert!(matches!(
            verify::<_, C10lCfg>(&mut ctx, query, data_pack.leash, data_pack.signature)
                .unwrap_err(),
            Error::InvalidSignedSimulateCall("current block out of range")
        ));
    }

    #[test]
    fn test_decode_simulate_call_query() {
        let (unsigned_body, data_pack) = make_signed_call();
        let signed_body = SimulateCallQuery {
            data: cbor::to_vec(data_pack.clone()),
            ..unsigned_body
        };

        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx();

        let mut c10l_mock = mock::Mock::default();
        c10l_mock.runtime_header.round = data_pack.leash.block_number;
        let mut c10l_ctx = c10l_mock.create_ctx();

        setup_nonce(&signed_body.caller, &data_pack.leash);
        setup_block(&data_pack.leash);

        let mut non_c10l_decode = |body: &SimulateCallQuery| {
            EVMModule::<Cfg>::decode_simulate_call_query(&mut ctx, body.clone())
        };
        let mut c10l_decode = |body: &SimulateCallQuery| {
            EVMModule::<C10lCfg>::decode_simulate_call_query(&mut c10l_ctx, body.clone())
        };

        assert!(EVMModule::<C10lCfg>::decode_simulate_call_query(
            &mut mock::Mock::default().create_ctx(),
            signed_body.clone()
        )
        .is_err()); // Check that errors are propagated (in this case leash invalidity).

        assert_eq!(c10l_decode(&signed_body).unwrap().0, unsigned_body);
        assert_eq!(non_c10l_decode(&unsigned_body).unwrap().0, unsigned_body);
    }
}
