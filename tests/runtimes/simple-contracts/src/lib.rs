//! Simple WASM contracts runtime.
use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    self as sdk, config, keymanager::TrustedPolicySigners, modules, types::token::Denomination,
    Version,
};
use oasis_runtime_sdk_contracts as contracts;

pub struct Runtime;

/// Runtime configuration.
pub struct Config;

impl modules::core::Config for Config {
    const ALLOW_INTERACTIVE_READ_ONLY_TRANSACTIONS: bool = true;
}

impl contracts::Config for Config {
    type Accounts = modules::accounts::Module;
}

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
        modules::core::Module<Config>,
        contracts::Module<Config>,
    );

    fn trusted_policy_signers() -> Option<TrustedPolicySigners> {
        Some(TrustedPolicySigners::default())
    }

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            modules::accounts::Genesis {
                parameters: Default::default(),
                balances: {
                    let mut b = BTreeMap::new();
                    // Alice.
                    b.insert(sdk::testing::keys::alice::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 10_000_000);
                        d
                    });
                    // Dave.
                    b.insert(sdk::testing::keys::dave::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 100_000_000);
                        d
                    });
                    b
                },
                total_supplies: {
                    let mut ts = BTreeMap::new();
                    ts.insert(Denomination::NATIVE, 110_000_000);
                    ts
                },
                ..Default::default()
            },
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 1_000_000_000,
                    max_tx_size: 512 * 1024,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: modules::core::GasCosts {
                        auth_signature: 0,
                        auth_multisig_signer: 0,
                        ..Default::default()
                    },
                    min_gas_price: {
                        let mut mgp = BTreeMap::new();
                        mgp.insert(Denomination::NATIVE, 0);
                        mgp
                    },
                },
            },
            contracts::Genesis {
                parameters: Default::default(),
            },
        )
    }
}
