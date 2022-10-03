//! Minimal runtime.
use std::collections::BTreeMap;

use oasis_runtime_sdk::{self as sdk, modules, types::token::Denomination, Version};

/// Configuration of the various modules.
pub struct Config;

// The base runtime type.
//
// Note that everything is statically defined, so the runtime has no state.
pub struct Runtime;

impl modules::core::Config for Config {}

impl sdk::Runtime for Runtime {
    // Use the crate version from Cargo.toml as the runtime version.
    const VERSION: Version = sdk::version_from_cargo!();

    // Define the module that provides the core API.
    type Core = modules::core::Module<Config>;

    // Define the modules that the runtime will be composed of. Here we just use
    // the core and accounts modules from the SDK. Later on we will go into
    // detail on how to create your own modules.
    type Modules = (modules::core::Module<Config>, modules::accounts::Module);

    // Define the genesis (initial) state for all of the specified modules. This
    // state is used when the runtime is first initialized.
    //
    // The return value is a tuple of states in the same order as the modules
    // are defined above.
    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            // Core module.
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000,
                    max_tx_signers: 8,
                    max_tx_size: 10_000,
                    max_multisig_signers: 8,
                    min_gas_price: BTreeMap::from([(Denomination::NATIVE, 0)]),
                    ..Default::default()
                },
            },
            // Accounts module.
            modules::accounts::Genesis {
                parameters: modules::accounts::Parameters {
                    gas_costs: modules::accounts::GasCosts { tx_transfer: 100 },
                    ..Default::default()
                },
                balances: BTreeMap::from([
                    (
                        sdk::testing::keys::alice::address(),
                        BTreeMap::from([(Denomination::NATIVE, 1_000_000_000)]),
                    ),
                    (
                        sdk::testing::keys::bob::address(),
                        BTreeMap::from([(Denomination::NATIVE, 2_000_000_000)]),
                    ),
                ]),
                total_supplies: BTreeMap::from([(Denomination::NATIVE, 3_000_000_000)]),
                ..Default::default()
            },
        )
    }
}
