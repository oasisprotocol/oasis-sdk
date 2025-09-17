use std::collections::{BTreeMap, BTreeSet};

use base64::prelude::*;
use tiny_keccak::{Hasher, TupleHash};

use oasis_runtime_sdk::types::address::Address;
use oasis_runtime_sdk_rofl_market::types::{Deployment, Instance};

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

/// Metadata key used to configure custom domains for the deployment.
pub const METADATA_KEY_PROXY_CUSTOM_DOMAINS: &str = "net.oasis.proxy.custom_domains";

/// Domain verification token context.
pub const DOMAIN_VERIFICATION_TOKEN_CONTEXT: &[u8] =
    b"rofl-scheduler/proxy: domain verification token";

/// Derive the verification token for a given domain.
///
/// The token is derived using a cryptographic hash function and is used to verify
/// that the domain is owned by the instance. More specifically, the token is derived
/// as follows:
///
///   TupleHash[DOMAIN_VERIFICATION_TOKEN_CONTEXT](
///     deployment.app_id,
///     instance.provider,
///     instance.id,
///     domain
///   )
///
/// The result is then encoded using base64.
pub fn domain_verification_token(
    instance: &Instance,
    deployment: &Deployment,
    domain: &str,
) -> String {
    let mut hasher = TupleHash::v256(DOMAIN_VERIFICATION_TOKEN_CONTEXT);
    hasher.update(deployment.app_id.as_ref());
    hasher.update(instance.provider.as_ref());
    hasher.update(instance.id.as_ref());
    hasher.update(domain.as_bytes());

    let mut output = Vec::new();
    hasher.finalize(&mut output);
    BASE64_STANDARD.encode(output)
}

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
