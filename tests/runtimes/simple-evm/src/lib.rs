//! Simple EVM runtime.
use std::collections::BTreeMap;

#[cfg(feature = "confidential")]
use oasis_runtime_sdk::keymanager::TrustedPolicySigners;
use oasis_runtime_sdk::{self as sdk, config, modules, types::token::Denomination, Version};
use oasis_runtime_sdk_evm as evm;

/// Simple EVM runtime.
pub struct Runtime;

/// Runtime configuration.
pub struct Config;

impl modules::core::Config for Config {}

impl evm::Config for Config {
    type Accounts = modules::accounts::Module;

    const CHAIN_ID: u64 = 0xa515;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;

    const CONFIDENTIAL: bool = cfg!(feature = "confidential");
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
        evm::Module<Config>,
    );

    #[cfg(feature = "confidential")]
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
                    max_batch_gas: 2_000_000,
                    max_tx_size: 32 * 1024,
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
            evm::Genesis {
                parameters: evm::Parameters {
                    gas_costs: Default::default(),
                },
            },
        )
    }
}
