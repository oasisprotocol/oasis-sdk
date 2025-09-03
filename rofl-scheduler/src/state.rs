use std::{collections::BTreeMap, time::Instant};

use base64::{prelude::BASE64_STANDARD, Engine};
use oasis_runtime_sdk::types::address::Address;
use oasis_runtime_sdk_rofl_market::types::{Deployment, Instance};

use crate::types::{Permissions, METADATA_KEY_PERMISSIONS};

/// Instance state.
#[derive(Debug, Clone)]
pub struct InstanceState {
    /// Address of the instance administrator.
    admin: Address,
    /// Permissions.
    permissions: Permissions,
    /// Last deployment.
    pub last_deployment: Option<Deployment>,
    /// Last error message corresponding to deploying `last_deployment`.
    pub last_error: Option<String>,
    // Whether to ignore instance start until the given time elapses.
    pub ignore_start_until: Option<Instant>,
    /// Backoff associated with ignoring instance start.
    pub ignore_start_backoff: Option<backoff::ExponentialBackoff>,
}

impl InstanceState {
    /// Create a new instance state.
    pub fn new(admin: Address, permissions: Permissions) -> Self {
        Self {
            admin,
            permissions,
            last_deployment: None,
            last_error: None,
            ignore_start_until: None,
            ignore_start_backoff: None,
        }
    }

    /// Create a new instance state from the given instance and deployment.
    pub fn from_instance(instance: &Instance, deployment: Option<&Deployment>) -> Self {
        let mut state = Self::new(instance.admin, Default::default());
        state.update_permissions(deployment);
        state
    }

    /// Update instance admin.
    pub fn update_admin(&mut self, instance: &Instance) {
        self.admin = instance.admin;
    }

    /// Update configured instance permissions from given deployment.
    pub fn update_permissions(&mut self, deployment: Option<&Deployment>) {
        self.permissions = deployment
            .and_then(|d| Self::parse_permissions_metadata(&d.metadata))
            .unwrap_or_default();
    }

    /// Check whether a given address has permission to perform the given action.
    pub fn has_permission(&self, action: &str, address: Address) -> bool {
        // An administrator always has permission to perform all actions.
        if self.admin == address {
            return true;
        }

        match self.permissions.get(action) {
            Some(addresses) => addresses.contains(&address),
            None => false,
        }
    }

    /// Parse permissions from deployment metadata.
    fn parse_permissions_metadata(metadata: &BTreeMap<String, String>) -> Option<Permissions> {
        let permissions = metadata.get(METADATA_KEY_PERMISSIONS)?;
        Self::parse_permissions_base64(permissions)
    }

    /// Parse permissions from a Base64-encoded string.
    fn parse_permissions_base64(permissions: &str) -> Option<Permissions> {
        let permissions = BASE64_STANDARD.decode(permissions).ok()?;
        cbor::from_slice(&permissions).ok()
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use oasis_runtime_sdk::testing;

    use super::*;

    #[test]
    fn test_permissions() {
        let mut permissions: Permissions = Default::default();
        permissions.insert(
            "log.view".to_owned(),
            BTreeSet::from([
                testing::keys::alice::address(),
                testing::keys::bob::address(),
            ]),
        );

        // Test parsing.
        let data = BASE64_STANDARD.encode(cbor::to_vec(permissions.clone()));
        let metadata: BTreeMap<String, String> =
            BTreeMap::from([("net.oasis.scheduler.permissions".to_string(), data)]);
        let parsed = InstanceState::parse_permissions_metadata(&metadata).unwrap();
        assert_eq!(permissions, parsed);

        let state = InstanceState::new(testing::keys::charlie::address(), parsed);
        let tcs = vec![
            ("foo.bar", testing::keys::charlie::address(), true),
            ("foo.bar", testing::keys::erin::address(), false),
            ("log.view", testing::keys::charlie::address(), true),
            ("log.view", testing::keys::alice::address(), true),
            ("log.clear", testing::keys::alice::address(), false),
            ("log.view", testing::keys::bob::address(), true),
            ("log.view", testing::keys::erin::address(), false),
        ];
        for tc in tcs {
            assert_eq!(state.has_permission(tc.0, tc.1), tc.2);
        }
    }
}
