//! Simple consensus runtime.
use std::collections::BTreeMap;

use oasis_runtime_sdk::{self as sdk, config, modules, types::token::Denomination, Version};

pub struct Config;

impl modules::core::Config for Config {}

/// Simple consensus runtime.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    const SCHEDULE_CONTROL: config::ScheduleControl = config::ScheduleControl {
        initial_batch_size: 50,
        batch_size: 50,
        min_remaining_gas: 100,
        max_tx_count: 1000,
    };

    type Core = modules::core::Module<Config>;

    type Modules = (
        modules::accounts::Module,
        modules::consensus::Module,
        modules::consensus_accounts::Module<modules::accounts::Module, modules::consensus::Module>,
        modules::core::Module<Config>,
    );

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            modules::accounts::Genesis {
                parameters: modules::accounts::Parameters {
                    denomination_infos: {
                        let mut denomination_infos = BTreeMap::new();
                        denomination_infos.insert(
                            "TEST".parse().unwrap(),
                            modules::accounts::types::DenominationInfo {
                                decimals: 12, // Consensus layer has 9 and we use a scaling factor of 1000.
                            },
                        );
                        denomination_infos
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            modules::consensus::Genesis {
                parameters: modules::consensus::Parameters {
                    consensus_denomination: "TEST".parse().unwrap(),
                    // Test scaling consensus base units when transferring them into the runtime.
                    consensus_scaling_factor: 1000,
                },
            },
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
                    max_tx_size: 32 * 1024,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    // These are free, in order to simplify testing.
                    gas_costs: Default::default(),
                    min_gas_price: {
                        let mut mgp = BTreeMap::new();
                        mgp.insert(Denomination::NATIVE, 0);
                        mgp
                    },
                },
            },
        )
    }

    fn migrate_state<C: sdk::Context>(_ctx: &mut C) {
        // Make sure that there are no spurious state migration invocations.
        panic!("state migration called when it shouldn't be");
    }
}
