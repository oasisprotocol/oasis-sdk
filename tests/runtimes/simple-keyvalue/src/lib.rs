//! Simple keyvalue runtime.
use std::collections::{BTreeMap, HashSet};

use oasis_runtime_sdk::{
    self as sdk, config,
    core::common::crypto::signature::PrivateKey,
    keymanager::TrustedPolicySigners,
    modules,
    types::token::{BaseUnits, Denomination},
    Module as _, Version,
};

pub mod keyvalue;
#[cfg(test)]
mod test;

/// Simple keyvalue runtime.
pub struct Runtime;

/// Runtime configuration.
pub struct Config;

impl modules::core::Config for Config {}

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    // Force an immediate migration. This is not what you would usually do immediately on genesis
    // but only in a later version when you need to update some of the parameters. We do this here
    // to test the migration functionality.
    const STATE_VERSION: u32 = 1;

    // Enable the runtime schedule control feature.
    const SCHEDULE_CONTROL: Option<config::ScheduleControl> = Some(config::ScheduleControl {
        initial_batch_size: 2,
        batch_size: 50,
        min_remaining_gas: 100,
        max_tx_count: 1000,
    });

    type Core = modules::core::Module<Config>;

    type Modules = (
        keyvalue::Module,
        modules::accounts::Module,
        modules::rewards::Module<modules::accounts::Module>,
        modules::core::Module<Config>,
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
                        confidential_insert_absent: 300,
                        confidential_insert_existing: 200,
                        confidential_remove_absent: 200,
                        confidential_remove_existing: 100,
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
                        d.insert(Denomination::NATIVE, 100_003_000);
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
                    ts.insert(Denomination::NATIVE, 100_016_100);
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
                    max_batch_gas: 2_000,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: modules::core::GasCosts {
                        tx_byte: 1,
                        auth_signature: 10,
                        auth_multisig_signer: 10,
                        callformat_x25519_deoxysii: 50,
                    },
                    min_gas_price: {
                        let mut mgp = BTreeMap::new();
                        mgp.insert(Denomination::NATIVE, 0);
                        mgp
                    },
                },
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
