//! Simple consensus runtime.
use std::collections::BTreeMap;

use oasis_runtime_sdk::{self as sdk, modules, types::token::Denomination, Version};

pub mod runtime;

/// Configuration of the various modules.
pub struct Config;

/// Benchmarking runtime.
pub struct Runtime;

impl modules::core::Config for Config {}

impl oasis_runtime_sdk_evm::Config for Config {
    type Accounts = modules::accounts::Module;

    const CHAIN_ID: u64 = 123456;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;
}

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    type Core = modules::core::Module<Config>;

    #[allow(clippy::type_complexity)]
    type Modules = (
        // Core.
        modules::core::Module<Config>,
        // Accounts.
        modules::accounts::Module,
        // Consensus layer interface.
        modules::consensus::Module,
        // Consensus layer accounts.
        modules::consensus_accounts::Module<modules::accounts::Module, modules::consensus::Module>,
        // EVM.
        oasis_runtime_sdk_evm::Module<Config>,
        // Benchmarks.
        runtime::Module<modules::accounts::Module>,
    );

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000_000,
                    max_tx_size: 32 * 1024,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    // These are free, in order to simplify benchmarking.
                    gas_costs: Default::default(),
                    min_gas_price: {
                        let mut mgp = BTreeMap::new();
                        mgp.insert(Denomination::NATIVE, 0);
                        mgp
                    },
                },
            },
            modules::accounts::Genesis {
                parameters: modules::accounts::Parameters {
                    debug_disable_nonce_check: true,
                    denomination_infos: {
                        let mut denomination_infos = BTreeMap::new();
                        denomination_infos.insert(
                            Denomination::NATIVE,
                            modules::accounts::types::DenominationInfo {
                                // Consistent with EVM ecosystem.
                                decimals: 18,
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
                    // Consensus layer denomination is the native denomination of this runtime.
                    consensus_denomination: Denomination::NATIVE,
                    // Scale to 18 decimal places as this is what is expected in the EVM ecosystem.
                    consensus_scaling_factor: 1_000_000_000,
                },
            },
            Default::default(),
            Default::default(),
            Default::default(),
        )
    }
}
