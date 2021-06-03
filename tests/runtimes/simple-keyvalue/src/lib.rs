//! Simple keyvalue runtime.
use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    self as sdk, modules,
    types::token::{BaseUnits, Denomination},
    Version,
};

pub mod keyvalue;
#[cfg(test)]
mod test;

/// Simple keyvalue runtime.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    type Modules = (
        keyvalue::Module,
        modules::accounts::Module,
        modules::rewards::Module<modules::accounts::Module>,
        modules::core::Module,
    );

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
                        d.insert(Denomination::NATIVE, 10_003_000.into());
                        d
                    });
                    // Bob.
                    b.insert(sdk::testing::keys::bob::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 2_000.into());
                        d
                    });
                    // Charlie.
                    b.insert(sdk::testing::keys::charlie::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 1_000.into());
                        d
                    });
                    // Dave.
                    b.insert(sdk::testing::keys::dave::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 100.into());
                        d
                    });
                    // Reward pool.
                    b.insert(*modules::rewards::ADDRESS_REWARD_POOL, {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 10_000.into());
                        d
                    });
                    b
                },
                total_supplies: {
                    let mut ts = BTreeMap::new();
                    ts.insert(Denomination::NATIVE, 10_016_100.into());
                    ts
                },
                ..Default::default()
            },
            modules::rewards::Genesis {
                parameters: modules::rewards::Parameters {
                    schedule: modules::rewards::types::RewardSchedule {
                        steps: vec![modules::rewards::types::RewardStep {
                            until: 1000,
                            amount: BaseUnits::new(100.into(), Denomination::NATIVE),
                        }],
                    },
                    participation_threshold_numerator: 3,
                    participation_threshold_denominator: 4,
                },
            },
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: modules::core::GasCosts {
                        auth_signature: 10,
                        auth_multisig_signer: 10,
                    },
                },
            },
        )
    }
}
