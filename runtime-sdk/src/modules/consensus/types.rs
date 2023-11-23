use oasis_core_runtime::common::namespace::Namespace;

/// Kind of root.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum RootKind {
    State = 1,
    IO = 2,
}

/// Internal round root call body.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct RoundRootBody {
    pub runtime_id: Namespace,
    pub round: u64,
    pub kind: RootKind,
}
