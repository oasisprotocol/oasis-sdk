use oasis_core_runtime::common::namespace::Namespace;

/// Kind of root.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[repr(u8)]
pub enum RootKind {
    #[default]
    Invalid = 0,
    State = 1,
    IO = 2,
}

impl RootKind {
    /// Whether the root kind is valid.
    pub fn is_valid(&self) -> bool {
        !matches!(self, Self::Invalid)
    }
}

/// Internal round root call body.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct RoundRootBody {
    pub runtime_id: Namespace,
    pub round: u64,
    pub kind: RootKind,
}
