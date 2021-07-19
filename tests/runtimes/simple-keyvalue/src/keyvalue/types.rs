//! Types for the keyvalue module.

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Key {
    pub key: Vec<u8>,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct KeyValue {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}
