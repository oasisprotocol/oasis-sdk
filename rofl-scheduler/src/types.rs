use std::collections::{BTreeMap, BTreeSet};

use oasis_runtime_sdk::types::address::Address;
use oasis_runtime_sdk_rofl_market::types::Deployment;

/// Name of the Deploy command.
pub const METHOD_DEPLOY: &str = "Deploy";
/// Name of the Restart command.
pub const METHOD_RESTART: &str = "Restart";
/// Name of the Terminate command.
pub const METHOD_TERMINATE: &str = "Terminate";

/// A command to be executed on a specific instance by the scheduler.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct Command {
    /// Method name.
    pub method: String,
    /// Method arguments.
    pub args: cbor::Value,
}

/// A deployment request.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DeployRequest {
    /// Deployment to be deployed into an instance.
    pub deployment: Deployment,
    /// Whether the storage should be wiped.
    pub wipe_storage: bool,
}

/// An instance restart request.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct RestartRequest {
    /// Whether the storage should be wiped.
    pub wipe_storage: bool,
}

/// An instance termination request.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct TerminateRequest {
    /// Whether the storage should be wiped.
    pub wipe_storage: bool,
}

/// Name of the metadata key used to store the delegated permissions.
pub const METADATA_KEY_PERMISSIONS: &str = "net.oasis.scheduler.permissions";

/// Name of the action for viewing machine logs.
pub const ACTION_LOG_VIEW: &str = "log.view";

/// Instance permissions for different actions.
pub type Permissions = BTreeMap<String, BTreeSet<Address>>;

#[cfg(test)]
mod test {
    use super::*;

    use oasis_runtime_sdk::testing;

    #[test]
    fn test_permissions_serialization() {
        let mut permissions: Permissions = Default::default();
        permissions.insert(
            ACTION_LOG_VIEW.to_owned(),
            BTreeSet::from([
                testing::keys::alice::address(),
                testing::keys::bob::address(),
            ]),
        );

        let data = cbor::to_vec(permissions.clone());
        let dec = cbor::from_slice(&data).unwrap();
        assert_eq!(permissions, dec);
    }
}
