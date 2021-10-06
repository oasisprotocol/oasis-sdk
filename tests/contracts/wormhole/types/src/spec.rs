//! Types defined by the wormhole protocol spec.
use std::{
    convert::{TryFrom, TryInto},
    io::{Cursor, Read},
    str,
};

use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use sha3::{Digest, Keccak256};

use oasis_contract_sdk::types::token;
use oasis_contract_sdk_types::{address::Address as SDKAddress, InstanceId};

use crate::Error;

/// Every chain part of the wormhole gets its own unique chain ID assigned.
// TODO: temporary value.
pub const OASIS_CHAIN_ID: u16 = 42;

/// Address is a Wormhole protocol address. It contains the native chain's address.
/// If the address data type of a chain is < 32 bytes, the value is zero-padded on the left.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(transparent)]
pub struct Address([u8; 32]);

impl Address {
    pub fn from_bytes(data: &[u8]) -> Result<Self, anyhow::Error> {
        if data.len() != 32 {
            return Err(anyhow!("invalid wormhole address length"));
        }

        let mut a = [0; 32];
        a.copy_from_slice(data);

        Ok(Address(a))
    }

    /// Return a byte representation of the wormhole address.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    pub fn as_oasis_address(&self) -> Result<SDKAddress, anyhow::Error> {
        SDKAddress::from_bytes(&self.0[11..]).map_err(|_| anyhow!("malformed sdk address"))
    }

    pub fn as_instance_id(&self) -> Result<InstanceId, anyhow::Error> {
        // TODO: ensure first 24 bytes are all zero.
        Ok(u64::from_be_bytes(self.0[24..].try_into().unwrap()).into())
    }
}

impl From<SDKAddress> for Address {
    fn from(addr: SDKAddress) -> Self {
        let mut result: Vec<u8> = vec![0; 11];
        result.extend(addr.as_ref());
        Self(result.try_into().unwrap())
    }
}

impl From<InstanceId> for Address {
    fn from(id: InstanceId) -> Self {
        let mut result: Vec<u8> = vec![0; 24];
        result.extend(id.as_u64().to_be_bytes());
        Self(result.try_into().unwrap())
    }
}

/// GuardianAddress is a Wormhole guardian address.
// For some reason these are 20-byte eth addresses and not zero-padded 32-byte ones.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(transparent)]
pub struct GuardianAddress([u8; 20]);

impl GuardianAddress {
    pub fn from_bytes(data: &[u8]) -> Result<Self, anyhow::Error> {
        if data.len() != 20 {
            return Err(anyhow!("invalid guardian address length"));
        }

        let mut ga = [0; 20];
        ga.copy_from_slice(data);

        Ok(GuardianAddress(ga))
    }
}

impl GuardianAddress {
    const ADDRESS_LEN: usize = 20;
}

/// Guardian set information.
#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub struct GuardianSet {
    /// Guardian set addresses.
    pub addresses: Vec<GuardianAddress>,
    /// Guardian set expiration time.
    pub expiration_time: u64,
}

impl GuardianSet {
    /// Returns number of guardians needed for quorum.
    pub fn quorum(&self) -> usize {
        ((self.addresses.len() * 10 / 3) * 2) / 10 + 1
    }
}

/// Governance VAA packet.
#[derive(Debug)]
pub struct GovernancePacket {
    pub module: String,
    pub action: GovernanceAction,
    pub chain: u16,
    pub payload: Vec<u8>,
}

impl GovernancePacket {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        let mut module = [0; 32];
        reader
            .read_exact(&mut module)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let module = str::from_utf8(&module)
            .map_err(|_| Error::InvalidVAAPayload)?
            .trim_end_matches(char::from(0))
            .to_string();

        let action = reader
            .read_u8()
            .map_err(|_| Error::InvalidVAAPayload)?
            .try_into()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let chain = reader
            .read_u16::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let mut payload = Vec::new();
        reader
            .read_to_end(&mut payload)
            .map_err(|_| Error::InvalidVAAPayload)?;

        Ok(GovernancePacket {
            module,
            action,
            chain,
            payload,
        })
    }
}

/// Governance actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum GovernanceAction {
    // 1 is reserved for upgrade / migrations.
    /// Update guardian set.
    UpdateGuardianSet = 2,
    /// Update fee.
    SetFee = 3,
    /// Transfer fee.
    TransferFee = 4,
}

impl TryFrom<u8> for GovernanceAction {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == GovernanceAction::UpdateGuardianSet as u8 => {
                Ok(GovernanceAction::UpdateGuardianSet)
            }
            x if x == GovernanceAction::SetFee as u8 => Ok(GovernanceAction::SetFee),
            x if x == GovernanceAction::TransferFee as u8 => Ok(GovernanceAction::TransferFee),
            _ => Err(()),
        }
    }
}

/// Governance action 2.
#[derive(Debug)]
pub struct GuardianSetUpgrade {
    pub new_guardian_set_index: u32,
    pub new_guardian_set: GuardianSet,
}

impl GuardianSetUpgrade {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        let new_guardian_set_index = reader
            .read_u32::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let n_guardians = reader.read_u8().map_err(|_| Error::InvalidVAAPayload)?;

        let mut addresses = vec![];
        for _ in 0..n_guardians {
            let mut buff = Vec::with_capacity(GuardianAddress::ADDRESS_LEN);
            reader
                .read_exact(&mut buff)
                .map_err(|_| Error::InvalidVAAPayload)?;

            addresses.push(GuardianAddress(
                buff.try_into().map_err(|_| Error::InvalidVAAAction)?,
            ));
        }

        let new_guardian_set = GuardianSet {
            addresses,
            expiration_time: 0,
        };

        Ok(GuardianSetUpgrade {
            new_guardian_set_index,
            new_guardian_set,
        })
    }
}

/// Governance action 3.
#[derive(Debug)]
pub struct SetFee {
    pub fee: token::BaseUnits,
}

impl SetFee {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        let _ = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;
        let amount = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let fee = token::BaseUnits::new(amount, token::Denomination::NATIVE);
        Ok(SetFee { fee })
    }
}

/// Governance action 4.
#[derive(Debug)]
pub struct TransferFee {
    pub amount: token::BaseUnits,
    pub recipient: SDKAddress,
}

impl TransferFee {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        // 32 bytes are reserved for addresses, but only the last 21 bytes are taken by the actual oasis address.
        let mut buff = [0; 32];
        reader
            .read_exact(&mut buff)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let recipient =
            SDKAddress::from_bytes(&buff[32 - 21..]).map_err(|_| Error::InvalidVAAPayload)?;

        let _ = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;
        let amount = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let amount = token::BaseUnits::new(amount, token::Denomination::NATIVE);
        Ok(TransferFee { amount, recipient })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct VAABody {
    pub timestamp: u32,
    pub nonce: u32,
    pub emitter_chain: u16,
    pub emitter_address: Address,
    pub sequence: u64,
    pub consistency_level: u8,
    pub payload: Vec<u8>,
}

impl VAABody {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![];
        buf.extend_from_slice(&self.timestamp.to_be_bytes());

        buf.extend_from_slice(&self.nonce.to_be_bytes());

        buf.extend_from_slice(&self.emitter_chain.to_be_bytes());

        buf.extend_from_slice(self.emitter_address.as_bytes());

        buf.extend_from_slice(&self.sequence.to_be_bytes());

        buf.extend_from_slice(&self.consistency_level.to_be_bytes());

        buf.extend_from_slice(&self.payload);

        buf
    }

    pub fn into_vaa(
        self,
        version: u8,
        guardian_set_index: u32,
        signatures: Vec<Vec<u8>>,
        hash: Vec<u8>,
    ) -> ParsedVAA {
        ParsedVAA {
            version,
            guardian_set_index,
            timestamp: self.timestamp,
            nonce: self.nonce,
            len_signers: signatures.len().try_into().unwrap(),
            signatures,
            emitter_chain: self.emitter_chain,
            emitter_address: self.emitter_address,
            sequence: self.sequence,
            consistency_level: self.consistency_level,
            payload: self.payload,
            hash,
        }
    }
}

// Validator Action Approval(VAA) data.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct ParsedVAA {
    pub version: u8,
    pub guardian_set_index: u32,
    pub timestamp: u32,
    pub nonce: u32,
    pub len_signers: u8,
    pub signatures: Vec<Vec<u8>>,

    pub emitter_chain: u16,
    pub emitter_address: Address,
    pub sequence: u64,
    pub consistency_level: u8,
    pub payload: Vec<u8>,

    pub hash: Vec<u8>,
}

impl ParsedVAA {
    /* VAA format:
    header (length 6):
    0   uint8   version (0x01)
    1   uint32  guardian set index
    5   uint8   len signatures
    per signature (length 66):
    0   uint8       index of the signer (in guardian keys)
    1   [65]uint8   signature
    body:
    0   uint32      timestamp (unix in seconds)
    4   uint32      nonce
    8   uint16      emitter_chain
    10  [32]uint8   emitter_address
    42  uint64      sequence
    50  uint8       consistency_level
    51  []uint8     payload
    */

    // Signature length and recovery id at the end.
    const SIG_DATA_LEN: usize = 65;

    pub fn deserialize(data: &[u8]) -> Result<Self, anyhow::Error> {
        let mut reader = Cursor::new(data);

        // Parse header.
        let version = reader.read_u8().map_err(|_| anyhow!("parsing version"))?;

        let guardian_set_index = reader
            .read_u32::<BigEndian>()
            .map_err(|_| anyhow!("parsing guardian set index"))?;

        let len_signers = reader
            .read_u8()
            .map_err(|_| anyhow!("parsing len signers"))?;

        // Parse signatures.
        let mut signatures = Vec::with_capacity(len_signers.into());
        let mut last_index = -1;
        for _ in 0..len_signers {
            let index = reader
                .read_u8()
                .map_err(|_| anyhow!("parsing guardian index"))? as i32;

            if index <= last_index {
                return Err(anyhow!("wrong guardian index order"));
            }
            last_index = index;

            let mut buffer = [0; Self::SIG_DATA_LEN];
            reader
                .read_exact(&mut buffer)
                .map_err(|_| anyhow!("parsing signature"))?;
            signatures.push(buffer.to_vec());
        }

        // Remember body offset position as we will later hash the body.
        let body_offset: usize = reader
            .position()
            .try_into()
            .map_err(|_| anyhow!("payload to big"))?;

        // Parse body.
        let timestamp = reader
            .read_u32::<BigEndian>()
            .map_err(|_| anyhow!("parsing timestamp"))?;

        let nonce = reader
            .read_u32::<BigEndian>()
            .map_err(|_| anyhow!("parsing nonce"))?;

        let emitter_chain = reader
            .read_u16::<BigEndian>()
            .map_err(|_| anyhow!("parsing emitter chain"))?;

        let mut buff = [0; 32];
        reader
            .read_exact(&mut buff)
            .map_err(|_| anyhow!("parsing emitter address"))?;
        let emitter_address = Address::from_bytes(&buff)?;

        let sequence = reader
            .read_u64::<BigEndian>()
            .map_err(|_| anyhow!("parsing sequence"))?;

        let consistency_level = reader
            .read_u8()
            .map_err(|_| anyhow!("parsing consistency level"))?;

        let mut payload = Vec::new();
        reader
            .read_to_end(&mut payload)
            .map_err(|_| anyhow!("parsing payload"))?;

        // Hash the body.
        let body = &data[body_offset..];
        let mut hasher = Keccak256::new();
        hasher.update(body);
        let hash = hasher.finalize().to_vec();

        // Rehash the hash
        let mut hasher = Keccak256::new();
        hasher.update(hash);
        let hash = hasher.finalize().to_vec();

        Ok(ParsedVAA {
            version,
            guardian_set_index,
            timestamp,
            nonce,
            len_signers: len_signers as u8,
            signatures,
            emitter_chain,
            emitter_address,
            sequence,
            consistency_level,
            payload,
            hash,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        // Header.
        let mut buf = self.version.to_be_bytes().to_vec();

        buf.extend_from_slice(&self.guardian_set_index.to_be_bytes());

        buf.extend_from_slice(&self.len_signers.to_be_bytes());

        for (index, signature) in self.signatures.iter().enumerate() {
            let index: u8 = index.try_into().unwrap();
            buf.extend_from_slice(&index.to_be_bytes());

            buf.extend_from_slice(signature);
        }

        // Body.
        buf.extend_from_slice(&self.timestamp.to_be_bytes());

        buf.extend_from_slice(&self.nonce.to_be_bytes());

        buf.extend_from_slice(&self.emitter_chain.to_be_bytes());

        buf.extend_from_slice(self.emitter_address.as_bytes());

        buf.extend_from_slice(&self.sequence.to_be_bytes());

        buf.extend_from_slice(&self.consistency_level.to_be_bytes());

        buf.extend_from_slice(&self.payload);

        buf
    }
}

#[cfg(test)]
mod test {
    use hex;

    use oasis_contract_sdk_types::testing::addresses;

    use super::*;

    #[test]
    fn test_wormhole_address() {
        let address = addresses::alice::address();
        let wormhole_address: Address = address.into();
        assert_eq!(
            wormhole_address.as_oasis_address().unwrap(),
            address,
            "sdk address to wormhole address round trip"
        );

        let instance_id = InstanceId::from(10);
        let wormhole_address: Address = instance_id.into();
        assert_eq!(
            wormhole_address.as_instance_id().unwrap(),
            instance_id,
            "instance id to wormhole address round trip"
        );
    }

    #[test]
    fn test_vaa_serialization() {
        // Taken from https://github.com/certusone/wormhole/blob/b577b70b2e2bd5104842362c63caaa3a363e2c00/terra/contracts/wormhole/src/state.rs#L434
        let x = hex::decode("080000000901007bfa71192f886ab6819fa4862e34b4d178962958d9b2e3d9437338c9e5fde1443b809d2886eaa69e0f0158ea517675d96243c9209c3fe1d94d5b19866654c6980000000b150000000500020001020304000000000000000000000000000000000000000000000000000000000000000000000a0261626364").unwrap();
        let v = ParsedVAA::deserialize(x.as_slice()).unwrap();
        assert_eq!(
            v,
            ParsedVAA {
                version: 8,
                guardian_set_index: 9,
                timestamp: 2837,
                nonce: 5,
                len_signers: 1,
                emitter_chain: 2,
                emitter_address: Address::from_bytes(&[
                    0, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0
                ])
                .unwrap(),
                sequence: 10,
                consistency_level: 2,
                payload: vec![97, 98, 99, 100],
                signatures: vec![vec![
                    123, 250, 113, 25, 47, 136, 106, 182, 129, 159, 164, 134, 46, 52, 180, 209,
                    120, 150, 41, 88, 217, 178, 227, 217, 67, 115, 56, 201, 229, 253, 225, 68, 59,
                    128, 157, 40, 134, 234, 166, 158, 15, 1, 88, 234, 81, 118, 117, 217, 98, 67,
                    201, 32, 156, 63, 225, 217, 77, 91, 25, 134, 102, 84, 198, 152, 0
                ]],
                // NOTE: different hash than in the upstream test:
                // https://github.com/certusone/wormhole/blob/b577b70b2e2bd5104842362c63caaa3a363e2c00/terra/contracts/wormhole/src/state.rs#L452-L455
                // Upstream hash is wrong, and is obtained by hashing the body only once - they probably updated the code but not the test.
                hash: vec![
                    164, 44, 82, 103, 33, 170, 183, 178, 188, 204, 35, 53, 78, 148, 160, 153, 122,
                    252, 84, 211, 26, 204, 128, 215, 37, 232, 222, 186, 222, 186, 98, 94
                ]
            },
            "deserialization should work",
        );

        assert_eq!(x, v.serialize(), "serialization should work")
    }
}
