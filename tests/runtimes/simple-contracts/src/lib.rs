//! Simple WASM contracts runtime.
use std::collections::{BTreeMap, HashSet};

use oasis_runtime_sdk::{
    self as sdk, config, core::common::crypto::signature::PrivateKey,
    keymanager::TrustedPolicySigners, modules, types::token::Denomination, Version,
};
use oasis_runtime_sdk_contracts as contracts;

/// Simple EVM runtime.
pub struct Runtime;

/// Runtime configuration.
pub struct Config;

impl modules::core::Config for Config {}

impl contracts::Config for Config {
    type Accounts = modules::accounts::Module;
}

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    // Enable the runtime schedule control feature.
    const SCHEDULE_CONTROL: Option<config::ScheduleControl> = Some(config::ScheduleControl {
        initial_batch_size: 50,
        batch_size: 50,
        min_remaining_gas: 100,
        max_tx_count: 1000,
    });

    type Core = modules::core::Module<Config>;

    type Modules = (
        modules::accounts::Module,
        modules::core::Module<Config>,
        contracts::Module<Config>,
    );

    fn trusted_policy_signers() -> Option<TrustedPolicySigners> {
        let signers = TrustedPolicySigners {
            signers: {
                let mut set = HashSet::new();
                for seed in [
                    "ekiden key manager test multisig key 0",
                    "ekiden key manager test multisig key 1",
                    "ekiden key manager test multisig key 2",
                ]
                .iter()
                {
                    let pk = PrivateKey::from_test_seed(seed.to_string());
                    set.insert(pk.public_key());
                }
                set
            },
            threshold: 2,
        };
        Some(signers)
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
                    max_batch_gas: 10_000_000,
                    max_in_msgs_gas: 2_500_000,
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
