use std::convert::TryInto;

use k256::{
    ecdsa::{recoverable, Signature},
    elliptic_curve::{sec1::ToEncodedPoint, IsHigh},
};
use thiserror::Error;

/// ECDSA signature verification/recovery errors.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("malformed input")]
    MalformedInput,
    #[error("malformed signature")]
    MalformedSignature,
    #[error("signature verification failed")]
    VerificationFailed,
}

/// Recover the ECDSA/secp256k1 public key used to create the given signature.
///
/// Only recovery IDs of 0 and 1 are supported.
/// This is the same restriction as in Ethereum (https://github.com/ethereum/go-ethereum/blob/v1.9.25/internal/ethapi/api.go#L466-L469).
///
/// Signatures with `s` scalar value greater than n/2 are rejected as well.
///
/// Returns the recovered pubkey in decompressed form.
pub fn recover(input: &[u8]) -> Result<[u8; 65], Error> {
    // [32] hash, [32] r, [32] s, [1] v.
    if input.len() != 97 {
        return Err(Error::MalformedInput);
    }

    let mut msg = [0u8; 32];
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    let v = input[96];

    msg[0..32].copy_from_slice(&input[0..32]);
    r[0..].copy_from_slice(&input[32..64]);
    s[0..].copy_from_slice(&input[64..96]);

    let signature = Signature::from_scalars(r, s).map_err(|_| Error::MalformedSignature)?;
    let signature = recoverable::Signature::new(
        &signature,
        recoverable::Id::new(v).map_err(|_| Error::MalformedSignature)?,
    )
    .map_err(|_| Error::MalformedSignature)?;

    if signature.s().is_high().into() {
        return Err(Error::MalformedSignature);
    }

    match signature.recover_verify_key_from_digest_bytes(&msg.into()) {
        Ok(recovered_key) => {
            let key = recovered_key.to_encoded_point(false);

            Ok(key.as_bytes().try_into().unwrap())
        }
        Err(_) => Err(Error::VerificationFailed),
    }
}

#[cfg(test)]
mod test {
    use hex;

    use super::*;

    #[test]
    fn test_ecdsa_recover() {
        let cases = [
            // https://github.com/ethereum/go-ethereum/blob/d8ff53dfb8a516f47db37dbc7fd7ad18a1e8a125/crypto/signature_test.go
            (
                hex::decode("ce0677bb30baa8cf067c88db9811f4333d131bf8bcf12fe7065d211dce971008").unwrap(),
                hex::decode("90f27b8b488db00b00606796d2987f6a5f59ae62ea05effe84fef5b8b0e549984a691139ad57a3f0b906637673aa2f63d1f55cb1a69199d4009eea23ceaddc9301").unwrap(),
                Ok(hex::decode("04e32df42865e97135acfb65f3bae71bdc86f4d49150ad6a440b6f15878109880a0a2b2667f7e725ceea70c673093bf67663e0312623c8e091b13cf2c0f11ef652").unwrap()),
            ),
            // https://github.com/ethereumjs/ethereumjs-util/blob/v6.1.0/test/index.js#L496.
            (
                hex::decode("82ff40c0a986c6a5cfad4ddf4c3aa6996f1a7837f9c398e17e5de5cbd5a12b28").unwrap(),
                hex::decode("99e71a99cb2270b8cac5254f9e99b6210c6c10224a1579cf389ef88b20a1abe9129ff05af364204442bdb53ab6f18a99ab48acc9326fa689f228040429e3ca6600").unwrap(),
                Ok(hex::decode("04b4ac68eff3a82d86db5f0489d66f91707e99943bf796ae6a2dcb2205c9522fa7915428b5ac3d3b9291e62142e7246d85ad54504fabbdb2bae5795161f8ddf259").unwrap()),
            ),
            // Same as previous but with an invalid recovery id.
            (
                hex::decode("82ff40c0a986c6a5cfad4ddf4c3aa6996f1a7837f9c398e17e5de5cbd5a12b28").unwrap(),
                hex::decode("99e71a99cb2270b8cac5254f9e99b6210c6c10224a1579cf389ef88b20a1abe9129ff05af364204442bdb53ab6f18a99ab48acc9326fa689f228040429e3ca6605").unwrap(),
                Err(Error::MalformedSignature),
            ),
            // https://github.com/randombit/botan/blob/2.9.0/src/tests/data/pubkey/ecdsa_key_recovery.vec
            // Malformed due to high-s.
            // Note: CosmWasm doesn't reject this one: https://github.com/CosmWasm/cosmwasm/blob/71b42fb77ef02705d655a96899b1a8b0f17a05e4/packages/crypto/src/secp256k1.rs#L331-L342
            (
                hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap(),
                hex::decode("E30F2E6A0F705F4FB5F8501BA79C7C0D3FAC847F1AD70B873E9797B17B89B39081F1A4457589F30D76AB9F89E748A68C8A94C30FE0BAC8FB5C0B54EA70BF6D2F00").unwrap(),
                Err(Error::MalformedSignature),
            ),
            // https://github.com/CosmWasm/cosmwasm/blob/71b42fb77ef02705d655a96899b1a8b0f17a05e4/packages/crypto/src/secp256k1.rs#L344
            (
                hex::decode("5ae8317d34d1e595e3fa7247db80c0af4320cce1116de187f8f7e2e099c0d8d0").unwrap(),
                hex::decode("45c0b7f8c09a9e1f1cea0c25785594427b6bf8f9f878a8af0b1abbb48e16d0920d8becd0c220f67c51217eecfd7184ef0732481c843857e6bc7fc095c4f6b78801").unwrap(),
                Ok(hex::decode("044a071e8a6e10aada2b8cf39fa3b5fb3400b04e99ea8ae64ceea1a977dbeaf5d5f8c8fbd10b71ab14cd561f7df8eb6da50f8a8d81ba564342244d26d1d4211595").unwrap()),
            ),
            // Invalid recovery params.
            // https://github.com/CosmWasm/cosmwasm/blob/71b42fb77ef02705d655a96899b1a8b0f17a05e4/packages/crypto/src/secp256k1.rs#L357
            (
                hex::decode("5ae8317d34d1e595e3fa7247db80c0af4320cce1116de187f8f7e2e099c0d8d0").unwrap(),
                hex::decode("45c0b7f8c09a9e1f1cea0c25785594427b6bf8f9f878a8af0b1abbb48e16d0920d8becd0c220f67c51217eecfd7184ef0732481c843857e6bc7fc095c4f6b78802").unwrap(),
                Err(Error::MalformedSignature),
            ),
            (
                hex::decode("5ae8317d34d1e595e3fa7247db80c0af4320cce1116de187f8f7e2e099c0d8d0").unwrap(),
                hex::decode("45c0b7f8c09a9e1f1cea0c25785594427b6bf8f9f878a8af0b1abbb48e16d0920d8becd0c220f67c51217eecfd7184ef0732481c843857e6bc7fc095c4f6b78803").unwrap(),
                Err(Error::MalformedSignature),
            ),
            (
                hex::decode("5ae8317d34d1e595e3fa7247db80c0af4320cce1116de187f8f7e2e099c0d8d0").unwrap(),
                hex::decode("45c0b7f8c09a9e1f1cea0c25785594427b6bf8f9f878a8af0b1abbb48e16d0920d8becd0c220f67c51217eecfd7184ef0732481c843857e6bc7fc095c4f6b78804").unwrap(),
                Err(Error::MalformedSignature),
            ),
            (
                hex::decode("5ae8317d34d1e595e3fa7247db80c0af4320cce1116de187f8f7e2e099c0d8d0").unwrap(),
                hex::decode("45c0b7f8c09a9e1f1cea0c25785594427b6bf8f9f878a8af0b1abbb48e16d0920d8becd0c220f67c51217eecfd7184ef0732481c843857e6bc7fc095c4f6b788ff").unwrap(),
                Err(Error::MalformedSignature),
            ),
            // Malformed msg.
            (
                hex::decode("ce0677bb30baa8cf067c88db9811f4333d131bf8bcf12fe7065d211dce97100800").unwrap(),
                hex::decode("90f27b8b488db00b00606796d2987f6a5f59ae62ea05effe84fef5b8b0e549984a691139ad57a3f0b906637673aa2f63d1f55cb1a69199d4009eea23ceaddc9301").unwrap(),
                Err(Error::MalformedInput),
            ),
        ];

        for (msg, sig, expected) in cases {
            let input = [msg.clone(), sig.clone()].concat();

            let output = recover(&input).map(|x| x.to_vec());
            assert_eq!(
                expected,
                output,
                "result should match expected result for msg {}, sig: {}",
                hex::encode(msg),
                hex::encode(sig),
            );
        }
    }
}
