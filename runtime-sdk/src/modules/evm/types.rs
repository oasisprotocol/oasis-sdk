//! EVM module types.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct CreateTx {
    pub value: Vec<u8>, // U256
    pub init_code: Vec<u8>,
    pub gas_limit: u64,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct CallTx {
    pub address: Vec<u8>, // H160
    pub value: Vec<u8>,   // U256
    pub data: Vec<u8>,
    pub gas_limit: u64,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct PeekStorageQuery {
    pub address: Vec<u8>, // H160
    pub index: Vec<u8>,   // H256
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct PeekCodeQuery {
    pub address: Vec<u8>, // H160
}

// The rest of the file contains wrappers for primitive_types::{H256, U256, H160},
// so that we can implement cbor::{Encode, Decode} for them, ugh.
// Remove this once oasis-cbor#8 is implemented.

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct H256(primitive_types::H256);

impl From<primitive_types::H256> for H256 {
    fn from(a: primitive_types::H256) -> H256 {
        H256(a)
    }
}

impl From<H256> for primitive_types::H256 {
    fn from(a: H256) -> primitive_types::H256 {
        a.0
    }
}

impl AsRef<[u8]> for H256 {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl cbor::Encode for H256 {
    fn into_cbor_value(self) -> cbor::Value {
        cbor::Value::ByteString(self.0.as_bytes().to_vec())
    }
}

impl cbor::Decode for H256 {
    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::ByteString(data) => Ok(Self::from_slice(&data)),
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}

impl H256 {
    pub fn from_slice(src: &[u8]) -> Self {
        H256(primitive_types::H256::from_slice(src))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct U256(primitive_types::U256);

impl From<primitive_types::U256> for U256 {
    fn from(a: primitive_types::U256) -> U256 {
        U256(a)
    }
}

impl From<U256> for primitive_types::U256 {
    fn from(a: U256) -> primitive_types::U256 {
        a.0
    }
}

impl cbor::Encode for U256 {
    fn into_cbor_value(self) -> cbor::Value {
        let mut bs = [0u8; 32];
        self.0.to_big_endian(&mut bs);
        cbor::Value::ByteString(bs.to_vec())
    }
}

impl cbor::Decode for U256 {
    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::ByteString(data) => {
                if data.len() != 32 {
                    Err(cbor::DecodeError::UnexpectedType)
                } else {
                    Ok(Self::from_big_endian(&data))
                }
            }
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}

impl U256 {
    pub fn from_big_endian(src: &[u8]) -> Self {
        U256(primitive_types::U256::from_big_endian(src))
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct H160(primitive_types::H160);

impl From<primitive_types::H160> for H160 {
    fn from(a: primitive_types::H160) -> H160 {
        H160(a)
    }
}

impl From<H160> for primitive_types::H160 {
    fn from(a: H160) -> primitive_types::H160 {
        a.0
    }
}

impl AsRef<[u8]> for H160 {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl cbor::Encode for H160 {
    fn into_cbor_value(self) -> cbor::Value {
        cbor::Value::ByteString(self.0.as_bytes().to_vec())
    }
}

impl cbor::Decode for H160 {
    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::ByteString(data) => Ok(Self::from_slice(&data)),
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}

impl H160 {
    pub fn from_slice(src: &[u8]) -> Self {
        H160(primitive_types::H160::from_slice(src))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
