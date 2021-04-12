//! Simple consensus runtime.
use oasis_runtime_sdk::{self as sdk, modules, Version};

/// Simple consensus runtime.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    const VERSION: Version = sdk::version_from_cargo!();

    type Modules = (
        modules::accounts::Module,
        modules::consensus_accounts::Module<modules::accounts::Module, modules::consensus::Module>,
    );

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        Default::default()
    }
}
