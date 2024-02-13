use std::{
    convert::{TryFrom, TryInto},
    io::{Cursor, Read},
    str,
};

use byteorder::{BigEndian, ReadBytesExt};

use oasis_wormhole_types as wormhole;

use crate::Error;

/// Computes internal wrapped asset ID from the assets chain ID and its address.
pub(crate) fn wrapped_asset_id(chain: u16, address: &wormhole::spec::Address) -> [u8; 34] {
    let mut asset_id = [0u8; 34];
    asset_id[0..2].copy_from_slice(&chain.to_be_bytes());
    asset_id[2..].copy_from_slice(address.as_bytes());
    asset_id
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
    // Register chain.
    RegisterChain = 1,
}

impl TryFrom<u8> for GovernanceAction {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == GovernanceAction::RegisterChain as u8 => Ok(GovernanceAction::RegisterChain),
            _ => Err(()),
        }
    }
}

/// Action payload used to register the token bridge contract (emitter address) for a foreign chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterChain {
    /// Emitter Chain ID.
    pub emitter_chain_id: u16,
    /// Emitter address.
    pub emitter_address: wormhole::spec::Address,
}

impl RegisterChain {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        let emitter_chain_id = reader
            .read_u16::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let mut address = Vec::new();
        reader
            .read_to_end(&mut address)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let emitter_address =
            wormhole::spec::Address::from_bytes(&address).map_err(|_| Error::InvalidVAAPayload)?;

        Ok(RegisterChain {
            emitter_chain_id,
            emitter_address,
        })
    }
}

/// Token bridge actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum TokenBridgeAction {
    // Transfer.
    Transfer = 1,
    // Attest meta.
    AttestMeta = 2,
}

impl TokenBridgeAction {
    fn to_be_bytes(self) -> [u8; 1] {
        (self as u8).to_be_bytes()
    }
}

impl TryFrom<u8> for TokenBridgeAction {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == TokenBridgeAction::Transfer as u8 => Ok(TokenBridgeAction::Transfer),
            x if x == TokenBridgeAction::AttestMeta as u8 => Ok(TokenBridgeAction::AttestMeta),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenBridgeMessage {
    pub action: TokenBridgeAction,
    pub payload: Vec<u8>,
}

impl TokenBridgeMessage {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        let action = reader
            .read_u8()
            .map_err(|_| Error::InvalidVAAPayload)?
            .try_into()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let mut payload = Vec::new();
        reader
            .read_to_end(&mut payload)
            .map_err(|_| Error::InvalidVAAPayload)?;

        Ok(TokenBridgeMessage { action, payload })
    }

    pub fn serialize(&self) -> Vec<u8> {
        [self.action.to_be_bytes().to_vec(), self.payload.clone()].concat()
    }
}

/// Action payload used to initiate a Transfer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferInfo {
    /// Amount being transferred.
    pub amount: (u128, u128),
    /// Address of the token.
    pub token_address: wormhole::spec::Address,
    /// Chain ID of the token.
    pub token_chain: u16,
    /// Address of the recipient.
    pub recipient: wormhole::spec::Address,
    /// Chain ID of the recipient.
    pub recipient_chain: u16,
    /// Amount of tokens that the user is willing to pay as relayer fee.
    pub fee: (u128, u128),
}

impl TransferInfo {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        let amnt1 = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;
        let amnt2 = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;
        let amount = (amnt1, amnt2);

        let mut buff = [0; 32];
        reader
            .read_exact(&mut buff)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let token_address =
            wormhole::spec::Address::from_bytes(&buff).map_err(|_| Error::InvalidVAAPayload)?;

        let token_chain = reader
            .read_u16::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let mut buff = [0; 32];
        reader
            .read_exact(&mut buff)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let recipient =
            wormhole::spec::Address::from_bytes(&buff).map_err(|_| Error::InvalidVAAPayload)?; // TODO: could extract to wormhole helper.

        let recipient_chain = reader
            .read_u16::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let fee1 = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;
        let fee2 = reader
            .read_u128::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;
        let fee = (fee1, fee2); // TODO: could extract to wormhole helper.

        Ok(TransferInfo {
            amount,
            token_address,
            token_chain,
            recipient,
            recipient_chain,
            fee,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        [
            &self.amount.0.to_be_bytes(),
            &self.amount.1.to_be_bytes(),
            self.token_address.as_bytes(),
            &self.token_chain.to_be_bytes(),
            self.recipient.as_bytes(),
            &self.recipient_chain.to_be_bytes(),
            &self.fee.0.to_be_bytes(),
            &self.fee.1.to_be_bytes(),
        ]
        .concat()
    }
}

/// Action payload used to attest asset metadata (required before the first transfer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetMeta {
    /// Address of the token.
    pub token_address: wormhole::spec::Address,
    /// Chain ID of the token.
    pub token_chain: u16,
    /// Number of decimals of the token.
    pub decimals: u8,
    /// Symbol of the token.
    pub symbol: String,
    /// Name of the token.
    pub name: String,
}

impl AssetMeta {
    pub fn deserialize(data: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(data);

        let mut buff = [0; 32];
        reader
            .read_exact(&mut buff)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let token_address =
            wormhole::spec::Address::from_bytes(&buff).map_err(|_| Error::InvalidVAAPayload)?;

        let token_chain = reader
            .read_u16::<BigEndian>()
            .map_err(|_| Error::InvalidVAAPayload)?;

        let decimals = reader.read_u8().map_err(|_| Error::InvalidVAAPayload)?;

        let mut buff = [0; 32];
        reader
            .read_exact(&mut buff)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let symbol = str::from_utf8(&buff)
            .map_err(|_| Error::InvalidVAAPayload)?
            .trim_end_matches(char::from(0))
            .to_string();

        let mut buff = [0; 32];
        reader
            .read_exact(&mut buff)
            .map_err(|_| Error::InvalidVAAPayload)?;
        let name = str::from_utf8(&buff)
            .map_err(|_| Error::InvalidVAAPayload)?
            .trim_end_matches(char::from(0))
            .to_string();

        Ok(AssetMeta {
            token_chain,
            token_address,
            decimals,
            symbol,
            name,
        })
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        let symbol = self.symbol.as_bytes();
        if symbol.len() > 32 {
            return Err(Error::InvalidVAAPayload);
        }
        let mut symbol_32 = [0; 32];
        symbol_32[..symbol.len()].copy_from_slice(symbol);

        let name = self.name.as_bytes();
        if name.len() > 32 {
            return Err(Error::InvalidVAAPayload);
        }
        let mut name_32 = [0; 32];
        name_32[..name.len()].copy_from_slice(name);

        Ok([
            self.token_address.as_bytes(),
            &self.token_chain.to_be_bytes(),
            &self.decimals.to_be_bytes(),
            &symbol_32,
            &name_32,
        ]
        .concat())
    }
}

#[cfg(test)]
mod test {
    use oasis_contract_sdk_types::testing::addresses;

    use super::*;

    #[test]
    fn test_asset_meta_serialization() {
        // XXX: no test vectors in wormhole repo.

        let meta = AssetMeta {
            token_address: addresses::alice::address().into(),
            token_chain: 42,
            decimals: 8,
            symbol: "WTEST".to_string(),
            name: "Wormhole test token".to_string(),
        };

        assert_eq!(
            AssetMeta::deserialize(&meta.serialize().unwrap()).unwrap(),
            meta,
            "asset meta should match after round-trip serialization"
        )
    }

    #[test]
    fn test_transfer_info_serialization() {
        // XXX: no test vectors in wormhole repo.

        let info = TransferInfo {
            amount: (43242, 1342345403),
            token_address: addresses::alice::address().into(),
            token_chain: 41,
            recipient: addresses::bob::address().into(),
            recipient_chain: 32,
            fee: (1123, 75632432),
        };

        assert_eq!(
            TransferInfo::deserialize(&info.serialize()).unwrap(),
            info,
            "transfer info should match after round-trip serialization"
        )
    }
}
