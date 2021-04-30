//! Simple consensus runtime.
use oasis_runtime_sdk::{self as sdk, modules, Version};

/// Simple consensus runtime.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    type Modules = (
        modules::accounts::Module,
        modules::consensus_accounts::Module<modules::accounts::Module, modules::consensus::Module>,
        modules::core::Module,
    );

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            Default::default(),
            modules::consensus_accounts::Genesis {
                parameters: modules::consensus_accounts::Parameters {
                    gas_costs: modules::consensus_accounts::GasCosts {
                        tx_deposit: 100,
                        tx_withdraw: 100,
                    },
                },
            },
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000,
                },
            },
        )
    }
}
