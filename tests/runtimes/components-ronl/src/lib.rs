//! Multi-component runtime, RONL component.
use std::collections::BTreeMap;

use once_cell::sync::Lazy;

#[cfg(feature = "debug-mock-sgx")]
use oasis_runtime_sdk::keymanager::TrustedSigners;
use oasis_runtime_sdk::{
    self as sdk, config, modules,
    modules::rofl::app_id::AppId,
    types::token::{BaseUnits, Denomination},
    Version,
};

pub mod oracle;

/// Multi-component runtime.
pub struct Runtime;

/// Runtime configuration.
pub struct Config;

/// Example ROFL application identifier.
pub static EXAMPLE_APP_ID: Lazy<AppId> = Lazy::new(|| AppId::from_global_name("example"));

impl modules::core::Config for Config {}

impl modules::rofl::Config for Config {
    /// Stake for creating a ROFL application.
    const STAKE_APP_CREATE: BaseUnits = BaseUnits::new(1_000, Denomination::NATIVE);
}

impl oracle::Config for Config {
    type Rofl = modules::rofl::Module<Config>;

    fn rofl_app_id() -> AppId {
        *EXAMPLE_APP_ID
    }
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
    type FeeProxy = modules::rofl::Module<Config>;

    type Modules = (
        modules::accounts::Module,
        modules::consensus::Module,
        modules::consensus_accounts::Module<modules::consensus::Module>,
        modules::core::Module<Config>,
        modules::rofl::Module<Config>,
        oracle::Module<Config>,
    );

    #[cfg(feature = "debug-mock-sgx")]
    fn trusted_signers() -> Option<TrustedSigners> {
        Some(TrustedSigners::unsafe_mock())
    }

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        use modules::rofl::policy::*;
        use sdk::core::common::sgx::{pcs, EnclaveIdentity, QuotePolicy};

        (
            modules::accounts::Genesis {
                parameters: Default::default(),
                balances: BTreeMap::from([
                    // Alice.
                    (
                        sdk::testing::keys::alice::address(),
                        BTreeMap::from([(Denomination::NATIVE, 10_000_000)]),
                    ),
                    // Dave.
                    (
                        sdk::testing::keys::dave::address(),
                        BTreeMap::from([(Denomination::NATIVE, 100_000_000)]),
                    ),
                ]),
                total_supplies: BTreeMap::from([(Denomination::NATIVE, 110_000_000)]),
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
            modules::consensus_accounts::Genesis::default(),
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 15_000_000,
                    max_tx_size: 128 * 1024,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: modules::core::GasCosts {
                        auth_signature: 0,
                        auth_multisig_signer: 0,
                        ..Default::default()
                    },
                    min_gas_price: BTreeMap::from([(Denomination::NATIVE, 0)]),
                    dynamic_min_gas_price: Default::default(),
                },
            },
            modules::rofl::Genesis {
                parameters: Default::default(),
                apps: vec![modules::rofl::types::AppConfig {
                    id: *EXAMPLE_APP_ID,
                    policy: AppAuthPolicy {
                        quotes: QuotePolicy {
                            ias: None,
                            pcs: Some(pcs::QuotePolicy {
                                tcb_validity_period: 30,
                                min_tcb_evaluation_data_number: 16,
                                ..Default::default()
                            }),
                        },
                        enclaves: vec![
                            // SHA256("simple-rofl")
                            EnclaveIdentity::fortanix_test(
                                "e1b2c5cfacb4e57f6a0892db30d02dd21836b380c75d9d6b9e9deeec4f55d1c5"
                                    .parse()
                                    .unwrap(),
                            ),
                        ],
                        endorsements: vec![AllowedEndorsement::ComputeRole],
                        fees: FeePolicy::EndorsingNodePays,
                        max_expiration: 2,
                    },
                    admin: None,
                    ..Default::default()
                }],
            },
            oracle::Genesis::default(),
        )
    }
}
