/// Observation call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Observation {
    /// Observation value.
    pub value: u128,
    /// Observation timestamp.
    pub ts: u64,
}

/// State for an observation round.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Round {
    /// Observations in this round.
    pub observations: Vec<Observation>,
}
