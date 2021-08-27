//! Storage-related types.
use std::convert::TryFrom;

/// Kind of the store to use.
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum StoreKind {
    Public = 0,
    Confidential = 1,
}

impl StoreKind {
    /// Prefix that should be used for the underlying store.
    pub fn prefix(&self) -> &'static [u8] {
        match self {
            StoreKind::Public => &[0x00],
            StoreKind::Confidential => &[0x01],
        }
    }
}

impl TryFrom<u32> for StoreKind {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(StoreKind::Public),
            1 => Ok(StoreKind::Confidential),
            _ => Err(value),
        }
    }
}
