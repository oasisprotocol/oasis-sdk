//! Configuration types.

/// Runtime schedule control configuration.
pub struct ScheduleControl {
    /// Size of the initial batch that the node should provide to the runtime.
    pub initial_batch_size: u32,
    /// Size of each extra batch that the runtime should fetch.
    pub batch_size: u32,
    /// Minimum amount of gas that needs to be remaining in a batch in order to still consider
    /// including new transactions.
    pub min_remaining_gas: u64,
    /// Maximum number of transactions that can go in a batch.
    ///
    /// This is only used as a last resort to avoid the batch going over the runtime's limit.
    pub max_tx_count: usize,
}

impl ScheduleControl {
    /// Construct a default schedule control configuration.
    pub const fn default() -> Self {
        Self {
            initial_batch_size: 50,
            batch_size: 50,
            min_remaining_gas: 1_000,
            max_tx_count: 1_000,
        }
    }
}
