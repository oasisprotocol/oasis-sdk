//! Simple keyvalue runtime.
use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    self as sdk, core::common::version::Version, modules, types::token::Denomination,
};

pub mod keyvalue;

/// Simple keyvalue runtime.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    const VERSION: Version = Version::new(0, 1, 0);

    type Modules = (
        keyvalue::Module,
        modules::accounts::Module,
        modules::core::Module,
    );

    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            keyvalue::Genesis {
                parameters: Default::default(),
            },
            modules::accounts::Genesis {
                balances: {
                    let mut b = BTreeMap::new();
                    // Alice.
                    b.insert(sdk::testing::keys::alice::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 3_000.into());
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
                    b
                },
                total_supplies: {
                    let mut ts = BTreeMap::new();
                    ts.insert(Denomination::NATIVE, 6_100.into());
                    ts
                },
                ..Default::default()
            },
            (),
        )
    }
}
