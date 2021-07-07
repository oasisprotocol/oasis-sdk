//! Simple consensus runtime.
use oasis_runtime_sdk::{self as sdk, modules, Version};

pub mod runtime;

/// Simple consensus runtime.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    type Modules = (
        modules::accounts::Module,
        runtime::Module<modules::accounts::Module>,
        modules::core::Module,
    );

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            modules::accounts::Genesis {
                parameters: modules::accounts::Parameters {
                    debug_disable_nonce_check: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            Default::default(),
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000_000,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    // These are free, in order to simplify benchmarking.
                    gas_costs: Default::default(),
                },
            },
        )
    }
}
