//! Simple EVM runtime.
use std::collections::BTreeMap;

#[cfg(feature = "confidential")]
use oasis_runtime_sdk::keymanager::TrustedSigners;
use oasis_runtime_sdk::{self as sdk, config, modules, types::token::Denomination, Version};
use oasis_runtime_sdk_evm as evm;

/// Simple EVM runtime.
pub struct Runtime;

/// Runtime configuration.
pub struct Config;

impl modules::core::Config for Config {}

impl evm::Config for Config {
    type AdditionalPrecompileSet = ();

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
    type Accounts = modules::accounts::Module;

    type Modules = (
        modules::accounts::Module,
        modules::consensus::Module,
        modules::consensus_accounts::Module<modules::consensus::Module>,
        modules::core::Module<Config>,
        evm::Module<Config>,
    );

    #[cfg(feature = "confidential")]
    fn trusted_signers() -> Option<TrustedSigners> {
        Some(TrustedSigners::unsafe_mock())
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
            modules::consensus::Genesis {
                parameters: modules::consensus::Parameters {
                    gas_costs: Default::default(),
                    consensus_denomination: Denomination::NATIVE,
                    // Test scaling consensus base units when transferring them into the runtime.
                    consensus_scaling_factor: 1000,
                    min_delegate_amount: 2,
                },
            },
            modules::consensus_accounts::Genesis {
                parameters: modules::consensus_accounts::Parameters {
                    // These are free, in order to simplify testing. We do test gas accounting
                    // with other methods elsewhere though.
                    gas_costs: Default::default(),
                    ..Default::default()
                },
            },
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 30_000_000,
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
                    dynamic_min_gas_price: Default::default(),
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
