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
                    // These are free, in order to simplify testing. We do test gas accounting
                    // with other methods elsewhere though.
                    gas_costs: Default::default(),
                },
            },
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    // These are free, in order to simplify testing.
                    gas_costs: Default::default(),
                },
            },
        )
    }

    fn migrate_state<C: sdk::Context>(_ctx: &mut C) {
        // Make sure that there are no spurious state migration invocations.
        panic!("state migration called when it shouldn't be");
    }
}
