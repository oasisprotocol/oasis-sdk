//! Simple keyvalue runtime.
use std::collections::{BTreeMap, HashSet};

use oasis_runtime_sdk::{
    self as sdk,
    keymanager::{PrivateKey, TrustedPolicySigners},
    modules,
    types::token::{BaseUnits, Denomination},
    Module as _, Version,
};
use oasis_runtime_sdk_contracts as contracts;

pub mod keyvalue;
#[cfg(test)]
mod test;

/// Simple keyvalue runtime.
pub struct Runtime;

/// Configuration for the contracts module.
pub struct ContractsConfig;

impl contracts::Config for ContractsConfig {
    type Accounts = modules::accounts::Module;
}

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    // Force an immediate migration. This is not what you would usually do immediately on genesis
    // but only in a later version when you need to update some of the parameters. We do this here
    // to test the migration functionality.
    const STATE_VERSION: u32 = 1;

    type Modules = (
        keyvalue::Module,
        modules::accounts::Module,
        modules::rewards::Module<modules::accounts::Module>,
        modules::core::Module,
        contracts::Module<ContractsConfig>,
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
            keyvalue::Genesis {
                parameters: keyvalue::Parameters {
                    gas_costs: keyvalue::GasCosts {
                        insert_absent: 200,
                        insert_existing: 100,
                        remove_absent: 100,
                        remove_existing: 50,
                    },
                },
            },
            modules::accounts::Genesis {
                parameters: modules::accounts::Parameters {
                    gas_costs: modules::accounts::GasCosts { tx_transfer: 100 },
                    ..Default::default()
                },
                balances: {
                    let mut b = BTreeMap::new();
                    // Alice.
                    b.insert(sdk::testing::keys::alice::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 10_003_000);
                        d
                    });
                    // Bob.
                    b.insert(sdk::testing::keys::bob::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 2_000);
                        d
                    });
                    // Charlie.
                    b.insert(sdk::testing::keys::charlie::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 1_000);
                        d
                    });
                    // Dave.
                    b.insert(sdk::testing::keys::dave::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 100);
                        d
                    });
                    // Reward pool.
                    b.insert(*modules::rewards::ADDRESS_REWARD_POOL, {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 10_000);
                        d
                    });
                    b
                },
                total_supplies: {
                    let mut ts = BTreeMap::new();
                    ts.insert(Denomination::NATIVE, 10_016_100);
                    ts
                },
                ..Default::default()
            },
            modules::rewards::Genesis {
                parameters: modules::rewards::Parameters {
                    schedule: modules::rewards::types::RewardSchedule {
                        steps: vec![modules::rewards::types::RewardStep {
                            until: 1000,
                            amount: BaseUnits::new(100, Denomination::NATIVE),
                        }],
                    },
                    participation_threshold_numerator: 1, // These are updated below.
                    participation_threshold_denominator: 1,
                },
            },
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000_000,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: modules::core::GasCosts {
                        auth_signature: 10,
                        auth_multisig_signer: 10,
                    },
                },
            },
            contracts::Genesis {
                parameters: Default::default(),
            },
        )
    }

    fn migrate_state<C: sdk::Context>(ctx: &mut C) {
        // Fetch current parameters.
        type Rewards = modules::rewards::Module<modules::accounts::Module>;
        let mut params = Rewards::params(ctx.runtime_state());

        // Update the participation threshold (one of the E2E tests checks this and would fail
        // if we don't do this).
        params.participation_threshold_numerator = 3;
        params.participation_threshold_denominator = 4;

        // Store parameters.
        Rewards::set_params(ctx.runtime_state(), params)
    }
}
