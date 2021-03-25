//! Simple keyvalue runtime.
use oasis_runtime_sdk::{self as sdk, core::common::version::Version};

pub mod keyvalue;

/// Simple keyvalue runtime.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    const VERSION: Version = Version::new(0, 1, 0);

    type Modules = keyvalue::Module;

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        keyvalue::Genesis {
            parameters: Default::default(),
        }
    }
}
