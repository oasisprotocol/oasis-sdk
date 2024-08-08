use std::convert::TryInto;

use anyhow::{anyhow, Context as _};
use ethereum::{self, EnvelopedDecodable};
use k256::elliptic_curve::scalar::IsHigh;

use oasis_runtime_sdk::{
    crypto::signature,
    types::{address, token, transaction},
};

use crate::types;

pub fn recover_low(
    sig: &k256::ecdsa::Signature,
    sig_recid: k256::ecdsa::RecoveryId,
    sig_hash: &primitive_types::H256,
) -> Result<k256::ecdsa::VerifyingKey, anyhow::Error> {
    if sig.s().is_high().into() {
        return Err(anyhow!("signature s high"));
    }
    k256::ecdsa::VerifyingKey::recover_from_prehash(
        sig_hash.as_fixed_bytes().as_ref(),
        sig,
        sig_recid,
    )
    .with_context(|| "recover verify key from digest")
}

pub fn decode(
    body: &[u8],
    expected_chain_id: Option<u64>,
    min_gas_price: u128,
    denom: &token::Denomination,
) -> Result<transaction::Transaction, anyhow::Error> {
    let (
        chain_id,
        sig,
        sig_recid,
        sig_hash,
        eth_action,
        eth_value,
        eth_input,
        eth_nonce,
        eth_gas_price,
        eth_gas_limit,
    ) = match ethereum::TransactionV2::decode(body)
        .map_err(|_| anyhow!("decoding transaction rlp"))?
    {
        ethereum::TransactionV2::Legacy(eth_tx) => {
            let sig = k256::ecdsa::Signature::from_scalars(
                eth_tx.signature.r().to_fixed_bytes(),
                eth_tx.signature.s().to_fixed_bytes(),
            )
            .with_context(|| "signature from_scalars")?;
            let sig_recid = k256::ecdsa::RecoveryId::from_byte(eth_tx.signature.standard_v())
                .ok_or(anyhow!("bad recovery id"))?;
            let message = ethereum::LegacyTransactionMessage::from(eth_tx);

            (
                message.chain_id.or(expected_chain_id),
                sig,
                sig_recid,
                message.hash(),
                message.action,
                message.value,
                message.input,
                message.nonce,
                message.gas_price,
                message.gas_limit,
            )
        }
        ethereum::TransactionV2::EIP2930(eth_tx) => {
            let sig = k256::ecdsa::Signature::from_scalars(
                eth_tx.r.to_fixed_bytes(),
                eth_tx.s.to_fixed_bytes(),
            )
            .with_context(|| "signature from_scalars")?;
            let sig_recid = k256::ecdsa::RecoveryId::new(eth_tx.odd_y_parity, false);
            let message = ethereum::EIP2930TransactionMessage::from(eth_tx);

            (
                Some(message.chain_id),
                sig,
                sig_recid,
                message.hash(),
                message.action,
                message.value,
                message.input,
                message.nonce,
                message.gas_price,
                message.gas_limit,
            )
        }
        ethereum::TransactionV2::EIP1559(eth_tx) => {
            let sig = k256::ecdsa::Signature::from_scalars(
                eth_tx.r.to_fixed_bytes(),
                eth_tx.s.to_fixed_bytes(),
            )
            .with_context(|| "signature from_scalars")?;
            let sig_recid = k256::ecdsa::RecoveryId::new(eth_tx.odd_y_parity, false);
            let message = ethereum::EIP1559TransactionMessage::from(eth_tx);

            if message.max_fee_per_gas < message.max_priority_fee_per_gas {
                return Err(anyhow!("invalid gas price"));
            }
            let base_fee_per_gas = min_gas_price.into();
            if message.max_fee_per_gas < base_fee_per_gas {
                return Err(anyhow!("gas price too low"));
            }

            let priority_fee_per_gas = std::cmp::min(
                message.max_priority_fee_per_gas,
                message.max_fee_per_gas.saturating_sub(base_fee_per_gas),
            );
            let effective_gas_price = priority_fee_per_gas.saturating_add(base_fee_per_gas);

            (
                Some(message.chain_id),
                sig,
                sig_recid,
                message.hash(),
                message.action,
                message.value,
                message.input,
                message.nonce,
                effective_gas_price,
                message.gas_limit,
            )
        }
    };
    if chain_id != expected_chain_id {
        return Err(anyhow!(
            "chain ID {:?}, expected {:?}",
            chain_id,
            expected_chain_id
        ));
    }
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
    let key = recover_low(&sig, sig_recid, &sig_hash)?;
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
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo {
                address_spec: transaction::AddressSpec::Signature(
                    address::SignatureAddressSpec::Secp256k1Eth(
                        signature::secp256k1::PublicKey::from_bytes(
                            k256::EncodedPoint::from(&key).as_bytes(),
                        )
                        .with_context(|| "sdk secp256k1 public key from bytes")?,
                    ),
                ),
                nonce,
            }],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(resolved_fee_amount, denom.clone()),
                gas: gas_limit,
                consensus_messages: 0, // Dynamic number of consensus messages, limited by gas.
                proxy: None,
            },
            ..Default::default()
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

    #[allow(clippy::too_many_arguments)]
    fn decode_expect_call(
        raw: &str,
        expected_chain_id: Option<u64>,
        expected_to: &str,
        expected_value: u128,
        expected_data: &str,
        expected_gas_limit: u64,
        expected_gas_price: u128,
        expected_from: &str,
        expected_nonce: u64,
        min_gas_price: u128,
    ) {
        let tx = decode(
            &Vec::from_hex(raw).unwrap(),
            expected_chain_id,
            min_gas_price,
            &token::Denomination::NATIVE,
        )
        .unwrap();
        println!("{:?}", &tx);
        assert_eq!(tx.call.method, "evm.Call");
        let body: types::Call = cbor::from_value(tx.call.body).unwrap();
        assert_eq!(body.address, types::H160::from_str(expected_to).unwrap());
        assert_eq!(body.value, types::U256::from(expected_value));
        assert_eq!(body.data, Vec::from_hex(expected_data).unwrap());
        assert_eq!(tx.auth_info.signer_info.len(), 1);
        assert_eq!(
            derive_caller::from_tx_auth_info(&tx.auth_info).unwrap(),
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

    #[allow(clippy::too_many_arguments)]
    fn decode_expect_create(
        raw: &str,
        expected_chain_id: Option<u64>,
        expected_value: u128,
        expected_init_code: &str,
        expected_gas_limit: u64,
        expected_gas_price: u128,
        expected_from: &str,
        expected_nonce: u64,
        min_gas_price: u128,
    ) {
        let tx = decode(
            &Vec::from_hex(raw).unwrap(),
            expected_chain_id,
            min_gas_price,
            &token::Denomination::NATIVE,
        )
        .unwrap();
        println!("{:?}", &tx);
        assert_eq!(tx.call.method, "evm.Create");
        let body: types::Create = cbor::from_value(tx.call.body).unwrap();
        assert_eq!(body.value, types::U256::from(expected_value));
        assert_eq!(body.init_code, Vec::from_hex(expected_init_code).unwrap());
        assert_eq!(tx.auth_info.signer_info.len(), 1);
        assert_eq!(
            derive_caller::from_tx_auth_info(&tx.auth_info).unwrap(),
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

    fn decode_expect_invalid(raw: &str, expected_chain_id: Option<u64>) {
        let e = decode(
            &Vec::from_hex(raw).unwrap(),
            expected_chain_id,
            0,
            &token::Denomination::NATIVE,
        )
        .unwrap_err();
        eprintln!("Decoding error (expected): {:?}", e);
    }

    fn decode_expect_from_mismatch(
        raw: &str,
        expected_chain_id: Option<u64>,
        unexpected_from: &str,
    ) {
        match decode(
            &Vec::from_hex(raw).unwrap(),
            expected_chain_id,
            0,
            &token::Denomination::NATIVE,
        ) {
            Ok(tx) => {
                assert_ne!(
                    derive_caller::from_tx_auth_info(&tx.auth_info).unwrap(),
                    types::H160::from_str(unexpected_from).unwrap(),
                );
            }
            Err(e) => {
                // Returning Err is fine too.
                eprintln!("Decoding error (expected): {:?}", e);
            }
        }
    }

    #[test]
    fn test_decode_basic() {
        // https://github.com/ethereum/tests/blob/v10.0/BasicTests/txtest.json
        let legacy_tx = "f86b8085e8d4a510008227109413978aee95f38490e9769c39b2773ed763d9cd5f872386f26fc10000801ba0eab47c1a49bf2fe5d40e01d313900e19ca485867d462fe06e139e3a536c6d4f4a014a569d327dcda4b29f74f93c0e9729d2f49ad726e703f9cd90dbb0fbf6649f1";
        decode_expect_call(
            legacy_tx,
            None,
            "13978aee95f38490e9769c39b2773ed763d9cd5f",
            10_000_000_000_000_000,
            "",
            10_000,
            1_000_000_000_000,
            // "cow" test account
            "cd2a3d9f938e13cd947ec05abc7fe734df8dd826",
            0,
            1_000,
        );
        decode_expect_call(
            legacy_tx,
            Some(1), // Legacy pre-EIP-155 transaction should work with any chain ID.
            "13978aee95f38490e9769c39b2773ed763d9cd5f",
            10_000_000_000_000_000,
            "",
            10_000,
            1_000_000_000_000,
            // "cow" test account
            "cd2a3d9f938e13cd947ec05abc7fe734df8dd826",
            0,
            1_000,
        );
        decode_expect_create(
            // We're using a transaction normalized from the original (below) to have low `s`.
            // f87f8085e8d4a510008227108080af6025515b525b600a37f260003556601b596020356000355760015b525b54602052f260255860005b525b54602052f21ba05afed0244d0da90b67cf8979b0f246432a5112c0d31e8d5eedd2bc17b171c694a0bb1035c834677c2e1185b8dc90ca6d1fa585ab3d7ef23707e1a497a98e752d1b
            "f87f8085e8d4a510008227108080af6025515b525b600a37f260003556601b596020356000355760015b525b54602052f260255860005b525b54602052f21ca05afed0244d0da90b67cf8979b0f246432a5112c0d31e8d5eedd2bc17b171c694a044efca37cb9883d1ee7a47236f3592df152931a930566933de2dc6e341c11426",
            None,
            0,
            "6025515b525b600a37f260003556601b596020356000355760015b525b54602052f260255860005b525b54602052f2",
            10_000,
            1_000_000_000_000,
            // "horse" test account
            "13978aee95f38490e9769c39b2773ed763d9cd5f",
            0,
            1_000,
        );
    }

    #[test]
    fn test_decode_chain_id() {
        // Test with mismatching expect_chain_id to exercise our check.
        decode_expect_invalid(
            // Taken from test_decode_types with chain ID of 1.
            "01f86301028203e882c35094cccccccccccccccccccccccccccccccccccccccc8080c080a0260f95e555a1282ef49912ff849b2007f023c44529dc8fb7ecca7693cccb64caa06252cf8af2a49f4cb76fd7172feaece05124edec02db242886b36963a30c2606",
            Some(5),
        );
    }

    #[test]
    fn test_decode_types() {
        // https://github.com/ethereum/tests/blob/v10.0/BlockchainTests/ValidBlocks/bcEIP1559/transType.json

        // Legacy.
        decode_expect_call(
            "f861018203e882c35094cccccccccccccccccccccccccccccccccccccccc80801ca021539ef96c70ab75350c594afb494458e211c8c722a7a0ffb7025c03b87ad584a01d5395fe48edb306f614f0cd682b8c2537537f5fd3e3275243c42e9deff8e93d",
            None,
            "cccccccccccccccccccccccccccccccccccccccc",
            0,
            "",
            50_000,
            1_000,
            "d02d72e067e77158444ef2020ff2d325f929b363",
            1,
            1_000,
        );

        // Legacy.
        decode_expect_call(
            "01f86301028203e882c35094cccccccccccccccccccccccccccccccccccccccc8080c080a0260f95e555a1282ef49912ff849b2007f023c44529dc8fb7ecca7693cccb64caa06252cf8af2a49f4cb76fd7172feaece05124edec02db242886b36963a30c2606",
            Some(1),
            "cccccccccccccccccccccccccccccccccccccccc",
            0,
            "",
            50_000,
            1_000,
            "d02d72e067e77158444ef2020ff2d325f929b363",
            2,
            1_000,
        );

        // EIP-1559
        // maxFeePerGas = 1000
        // maxPriorityFeePerGas = 100
        decode_expect_call(
            "02f8640103648203e882c35094cccccccccccccccccccccccccccccccccccccccc8080c001a08480e6848952a15ae06192b8051d213d689bdccdf8f14cf69f61725e44e5e80aa057c2af627175a2ac812dab661146dfc7b9886e885c257ad9c9175c3fcec2202e",
            Some(1),
            "cccccccccccccccccccccccccccccccccccccccc",
            0,
            "",
            50_000,
            500, // min(100, 1000 - 400) + 400
            "d02d72e067e77158444ef2020ff2d325f929b363",
            3,
            400,
        );
    }

    #[test]
    fn test_decode_verify() {
        // Altered signature, out of bounds r = n.
        decode_expect_invalid("f86b8085e8d4a510008227109413978aee95f38490e9769c39b2773ed763d9cd5f872386f26fc10000801ba0fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141a014a569d327dcda4b29f74f93c0e9729d2f49ad726e703f9cd90dbb0fbf6649f1", None);
        // Altered signature, high s.
        decode_expect_invalid("f86b8085e8d4a510008227109413978aee95f38490e9769c39b2773ed763d9cd5f872386f26fc10000801ca0eab47c1a49bf2fe5d40e01d313900e19ca485867d462fe06e139e3a536c6d4f4a0eb5a962cd82325b4d608b06c3f168d618b652f7440d8609ee6c4a37d10cff750", None);
        // Altered signature, s decreased by one.
        decode_expect_from_mismatch(
            "f86b8085e8d4a510008227109413978aee95f38490e9769c39b2773ed763d9cd5f872386f26fc10000801ba0eab47c1a49bf2fe5d40e01d313900e19ca485867d462fe06e139e3a536c6d4f4a014a569d327dcda4b29f74f93c0e9729d2f49ad726e703f9cd90dbb0fbf6649f0",
            None,
            "cd2a3d9f938e13cd947ec05abc7fe734df8dd826",
        );
    }
}
