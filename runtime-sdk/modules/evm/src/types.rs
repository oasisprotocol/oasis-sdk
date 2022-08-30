//! EVM module types.

/// Transaction body for creating an EVM contract.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Create {
    pub value: U256,
    pub init_code: Vec<u8>,
}

/// Transaction body for calling an EVM contract.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Call {
    pub address: H160,
    pub value: U256,
    pub data: Vec<u8>,
}

/// Transaction body for peeking into EVM storage.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct StorageQuery {
    pub address: H160,
    pub index: H256,
}

/// Transaction body for peeking into EVM code storage.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct CodeQuery {
    pub address: H160,
}

/// Transaction body for fetching EVM account's balance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct BalanceQuery {
    pub address: H160,
}

/// Transaction body for simulating an EVM call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct SimulateCallQuery {
    pub gas_price: U256,
    pub gas_limit: u64,
    pub caller: H160,
    pub address: H160,
    pub value: U256,
    pub data: Vec<u8>,
}

/// An envelope containing the encryption-enveloped data of a [`SimulateCallQuery`]
/// and a signature generated according to [EIP-712](https://eips.ethereum.org/EIPS/eip-712)
/// over the unmodified Eth call.
///
/// EIP-712 is used so that the signed message can be easily verified by the user.
/// MetaMask, for instance, shows each field as itself, whereas a standard `eth_personalSign`
/// would show an opaque CBOR-encoded [`SimulateCallQuery`].
///
/// The EIP-712 type parameters for a signed query are:
/// ```ignore
/// {
///   domain: {
///     name: 'oasis-runtime-sdk/evm: signed query',
///     version: '1.0.0',
///     chainId,
///   },
///   types: {
///     Call: [
///       { name: 'from', type: 'address' },
///       { name: 'to', type: 'address' },
///       { name: 'value', type: 'uint256' },
///       { name: 'gasPrice', type: 'uint256' },
///       { name: 'gasLimit', type: 'uint64' },
///       { name: 'data', type: 'bytes' },
///       { name: 'leash', type: 'Leash' },
///     ],
///     Leash: [
///       { name: 'nonce', type: 'uint64' },
///       { name: 'blockNumber', type: 'uint64' },
///       { name: 'blockHash', type: 'uint256' },
///       { name: 'blockRange', type: 'uint64' },
///     ],
///   },
/// }
/// ```
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct SignedCallDataPack {
    pub data: oasis_runtime_sdk::types::transaction::Call,
    pub leash: Leash,
    pub signature: [u8; 65],
}

#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Leash {
    /// The maximum account nonce that will be tolerated.
    pub nonce: u64,
    /// The base block number.
    pub block_number: u64,
    /// The expeced hash at `block_number`.
    pub block_hash: H256,
    /// The range of the leash past `block_number`.
    pub block_range: u64,
}

// The rest of the file contains wrappers for primitive_types::{H160, H256, U256},
// so that we can implement cbor::{Encode, Decode} for them, ugh.
// Remove this once oasis-cbor#8 is implemented.
//
// Thanks to Nick for providing the fancy macros below :)

// This `mod` exists solely to place an `#[allow(warnings)]` around the generated code.
#[allow(warnings)]
mod eth {
    use std::convert::TryFrom;

    use thiserror::Error;

    use super::*;

    #[derive(Error, Debug)]
    pub enum NoError {}

    macro_rules! construct_fixed_hash {
        ($name:ident($num_bytes:literal)) => {
            fixed_hash::construct_fixed_hash! {
                pub struct $name($num_bytes);
            }

            impl cbor::Encode for $name {
                fn into_cbor_value(self) -> cbor::Value {
                    cbor::Value::ByteString(self.as_bytes().to_vec())
                }
            }

            impl cbor::Decode for $name {
                fn try_default() -> Result<Self, cbor::DecodeError> {
                    Ok(Default::default())
                }

                fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
                    match value {
                        cbor::Value::ByteString(v) => {
                            if v.len() == $num_bytes {
                                Ok(Self::from_slice(&v))
                            } else {
                                Err(cbor::DecodeError::UnexpectedIntegerSize)
                            }
                        }
                        _ => Err(cbor::DecodeError::UnexpectedType),
                    }
                }
            }

            impl TryFrom<&[u8]> for $name {
                type Error = NoError;

                fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
                    Ok(Self::from_slice(bytes))
                }
            }
        };
    }

    macro_rules! construct_uint {
        ($name:ident($num_words:tt)) => {
            uint::construct_uint! {
                pub struct $name($num_words);
            }

            impl cbor::Encode for $name {
                fn into_cbor_value(self) -> cbor::Value {
                    let mut out = [0u8; $num_words * 8];
                    self.to_big_endian(&mut out);
                    cbor::Value::ByteString(out.to_vec())
                }
            }

            impl cbor::Decode for $name {
                fn try_default() -> Result<Self, cbor::DecodeError> {
                    Ok(Default::default())
                }

                fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
                    match value {
                        cbor::Value::ByteString(v) => {
                            if v.len() <= $num_words * 8 {
                                Ok(Self::from_big_endian(&v))
                            } else {
                                Err(cbor::DecodeError::UnexpectedIntegerSize)
                            }
                        }
                        _ => Err(cbor::DecodeError::UnexpectedType),
                    }
                }
            }
        };
    }

    construct_fixed_hash!(H160(20));
    construct_fixed_hash!(H256(32));
    construct_uint!(U256(4));

    macro_rules! impl_upstream_conversions {
        ($($ty:ident),* $(,)?) => {
            $(
                impl From<$ty> for primitive_types::$ty {
                    fn from(t: $ty) -> Self {
                        Self(t.0)
                    }
                }

                impl From<primitive_types::$ty> for $ty {
                    fn from(t: primitive_types::$ty) -> Self {
                        Self(t.0)
                    }
                }
            )*
        }
    }

    impl_upstream_conversions!(H160, H256, U256);
}
pub use eth::{H160, H256, U256};
