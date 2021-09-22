use std::convert::TryInto;

use anyhow::{anyhow, Context as _};
use ethereum;
use k256;

use oasis_runtime_sdk::{
    crypto::signature,
    types::{token, transaction},
};

use crate::types;

pub fn decode(body: &[u8]) -> Result<transaction::Transaction, anyhow::Error> {
    let (sig, sig_hash, eth_action, eth_value, eth_input, eth_nonce, eth_gas_price, eth_gas_limit) =
        match rlp::decode::<ethereum::TransactionV2>(body)
            .with_context(|| "decoding transaction rlp")?
        {
            ethereum::TransactionV2::Legacy(eth_tx) => {
                let sig = k256::ecdsa::recoverable::Signature::new(
                    &k256::ecdsa::Signature::from_scalars(
                        eth_tx.signature.r().to_fixed_bytes(),
                        eth_tx.signature.s().to_fixed_bytes(),
                    )
                    .with_context(|| "signature from_scalars")?,
                    k256::ecdsa::recoverable::Id::new(eth_tx.signature.standard_v())
                        .with_context(|| "recoverable id new")?,
                )
                .with_context(|| "recoverable signature new")?;
                let message = ethereum::LegacyTransactionMessage::from(eth_tx);
                let sig_hash = message.hash();
                (
                    sig,
                    sig_hash,
                    message.action,
                    message.value,
                    message.input,
                    message.nonce,
                    message.gas_price,
                    message.gas_limit,
                )
            }
            ethereum::TransactionV2::EIP2930(eth_tx) => {
                let sig = k256::ecdsa::recoverable::Signature::new(
                    &k256::ecdsa::Signature::from_scalars(
                        eth_tx.r.to_fixed_bytes(),
                        eth_tx.s.to_fixed_bytes(),
                    )
                    .with_context(|| "signature from_scalars")?,
                    k256::ecdsa::recoverable::Id::new(eth_tx.odd_y_parity.into())
                        .with_context(|| "recoverable id new")?,
                )
                .with_context(|| "recoverable signature new")?;
                let message = ethereum::EIP2930TransactionMessage::from(eth_tx);
                let sig_hash = message.hash();
                (
                    sig,
                    sig_hash,
                    message.action,
                    message.value,
                    message.input,
                    message.nonce,
                    message.gas_price,
                    message.gas_limit,
                )
            }
            ethereum::TransactionV2::EIP1559(eth_tx) => {
                let sig = k256::ecdsa::recoverable::Signature::new(
                    &k256::ecdsa::Signature::from_scalars(
                        eth_tx.r.to_fixed_bytes(),
                        eth_tx.s.to_fixed_bytes(),
                    )
                    .with_context(|| "signature from_scalars")?,
                    k256::ecdsa::recoverable::Id::new(eth_tx.odd_y_parity.into())
                        .with_context(|| "recoverable id new")?,
                )
                .with_context(|| "recoverable signature new")?;
                let message = ethereum::EIP1559TransactionMessage::from(eth_tx);
                let sig_hash = message.hash();
                // Base fee is zero. Allocate only priority fee.
                let resolved_gas_price =
                    std::cmp::min(message.max_fee_per_gas, message.max_priority_fee_per_gas);
                (
                    sig,
                    sig_hash,
                    message.action,
                    message.value,
                    message.input,
                    message.nonce,
                    resolved_gas_price,
                    message.gas_limit,
                )
            }
        };
    let (method, body) = match eth_action {
        ethereum::TransactionAction::Call(eth_address) => (
            "evm.Call",
            cbor::to_value(types::Call {
                address: eth_address.into(),
                value: eth_value.into(),
                data: eth_input,
            }),
        ),
        ethereum::TransactionAction::Create => (
            "evm.Create",
            cbor::to_value(types::Create {
                value: eth_value.into(),
                init_code: eth_input,
            }),
        ),
    };
    let key = sig
        .recover_verify_key_from_digest_bytes(sig_hash.as_bytes().into())
        .with_context(|| "recover verify key from digest")?;
    let nonce: u64 = eth_nonce
        .try_into()
        .map_err(|e| anyhow!("converting nonce: {}", e))?;
    let gas_price: u128 = eth_gas_price
        .try_into()
        .map_err(|e| anyhow!("converting gas price: {}", e))?;
    let gas_limit: u64 = eth_gas_limit
        .try_into()
        .map_err(|e| anyhow!("converting gas limit: {}", e))?;
    let resolved_fee_amount = gas_price
        .checked_mul(gas_limit as u128)
        .ok_or_else(|| anyhow!("computing total fee amount"))?;
    Ok(transaction::Transaction {
        version: transaction::LATEST_TRANSACTION_VERSION,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: method.to_owned(),
            body,
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo {
                address_spec: transaction::AddressSpec::Signature(signature::PublicKey::Secp256k1(
                    signature::secp256k1::PublicKey::from_bytes(
                        k256::EncodedPoint::from(&key).as_bytes(),
                    )
                    .with_context(|| "sdk secp256k1 public key from bytes")?,
                )),
                nonce,
            }],
            fee: transaction::Fee {
                amount: token::BaseUnits(resolved_fee_amount, token::Denomination::NATIVE),
                gas: gas_limit,
                consensus_messages: 0,
            },
        },
    })
}

#[cfg(test)]
mod test {
    use std::str::FromStr as _;

    use hex::FromHex as _;

    use oasis_runtime_sdk::types::token;

    use crate::{derive_caller, types};

    use super::decode;

    fn decode_expect_call(
        raw: &str,
        expected_to: &str,
        expected_value: u128,
        expected_data: &str,
        expected_gas_limit: u64,
        expected_gas_price: u128,
        expected_from: &str,
        expected_nonce: u64,
    ) {
        let tx = decode(&Vec::from_hex(raw).unwrap()).unwrap();
        println!("{:?}", &tx);
        assert_eq!(tx.call.method, "evm.Call");
        let body: types::Call = cbor::from_value(tx.call.body).unwrap();
        assert_eq!(body.address, types::H160::from_str(expected_to).unwrap());
        assert_eq!(body.value, types::U256::from(expected_value));
        assert_eq!(body.data, Vec::from_hex(expected_data).unwrap());
        assert_eq!(tx.auth_info.signer_info.len(), 1);
        assert_eq!(
            derive_caller::from_tx_auth_info(&tx.auth_info),
            types::H160::from_str(expected_from).unwrap(),
        );
        assert_eq!(tx.auth_info.signer_info[0].nonce, expected_nonce);
        assert_eq!(
            tx.auth_info.fee.amount.0,
            expected_gas_limit as u128 * expected_gas_price,
        );
        assert_eq!(tx.auth_info.fee.amount.1, token::Denomination::NATIVE);
        assert_eq!(tx.auth_info.fee.gas, expected_gas_limit);
    }

    fn decode_expect_create(
        raw: &str,
        expected_value: u128,
        expected_init_code: &str,
        expected_gas_limit: u64,
        expected_gas_price: u128,
        expected_from: &str,
        expected_nonce: u64,
    ) {
        let tx = decode(&Vec::from_hex(raw).unwrap()).unwrap();
        println!("{:?}", &tx);
        assert_eq!(tx.call.method, "evm.Create");
        let body: types::Create = cbor::from_value(tx.call.body).unwrap();
        assert_eq!(body.value, types::U256::from(expected_value));
        assert_eq!(body.init_code, Vec::from_hex(expected_init_code).unwrap());
        assert_eq!(tx.auth_info.signer_info.len(), 1);
        assert_eq!(
            derive_caller::from_tx_auth_info(&tx.auth_info),
            types::H160::from_str(expected_from).unwrap(),
        );
        assert_eq!(tx.auth_info.signer_info[0].nonce, expected_nonce);
        assert_eq!(
            tx.auth_info.fee.amount.0,
            expected_gas_limit as u128 * expected_gas_price,
        );
        assert_eq!(tx.auth_info.fee.amount.1, token::Denomination::NATIVE);
        assert_eq!(tx.auth_info.fee.gas, expected_gas_limit);
    }

    #[test]
    fn test_decode_basic() {
        // https://github.com/ethereum/tests/blob/v10.0/BasicTests/txtest.json
        decode_expect_call(
            "f86b8085e8d4a510008227109413978aee95f38490e9769c39b2773ed763d9cd5f872386f26fc10000801ba0eab47c1a49bf2fe5d40e01d313900e19ca485867d462fe06e139e3a536c6d4f4a014a569d327dcda4b29f74f93c0e9729d2f49ad726e703f9cd90dbb0fbf6649f1",
            "13978aee95f38490e9769c39b2773ed763d9cd5f",
            10_000_000_000_000_000,
            "",
            10_000,
            1_000_000_000_000,
            // "cow" test account
            "cd2a3d9f938e13cd947ec05abc7fe734df8dd826",
            0,
        );
        decode_expect_create(
            // We're using a transaction normalized from the original (below) to have low `s`.
            // f87f8085e8d4a510008227108080af6025515b525b600a37f260003556601b596020356000355760015b525b54602052f260255860005b525b54602052f21ba05afed0244d0da90b67cf8979b0f246432a5112c0d31e8d5eedd2bc17b171c694a0bb1035c834677c2e1185b8dc90ca6d1fa585ab3d7ef23707e1a497a98e752d1b
            "f87f8085e8d4a510008227108080af6025515b525b600a37f260003556601b596020356000355760015b525b54602052f260255860005b525b54602052f21ca05afed0244d0da90b67cf8979b0f246432a5112c0d31e8d5eedd2bc17b171c694a044efca37cb9883d1ee7a47236f3592df152931a930566933de2dc6e341c11426",
            0,
            "6025515b525b600a37f260003556601b596020356000355760015b525b54602052f260255860005b525b54602052f2",
            10_000,
            1_000_000_000_000,
            // "horse" test account
            "13978aee95f38490e9769c39b2773ed763d9cd5f",
            0,
        );
    }
}
