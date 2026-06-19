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

    let mut output = [0u8; 32]; // TupleHash::v256 → 256-bit token (matches keymanager.rs).
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

    mod domain_verification_token {
        use base64::prelude::*;
        use tiny_keccak::{Hasher, TupleHash};

        use oasis_runtime_sdk::{
            modules::rofl::app_id::AppId, testing::keys, types::address::Address,
        };
        use oasis_runtime_sdk_rofl_market::types::{Deployment, Instance};

        use super::super::{domain_verification_token, DOMAIN_VERIFICATION_TOKEN_CONTEXT};

        fn instance(provider: Address, id: u64) -> Instance {
            Instance {
                provider,
                id: id.into(),
                ..Default::default()
            }
        }

        fn deployment(app: &str) -> Deployment {
            Deployment {
                app_id: AppId::from_global_name(app),
                ..Default::default()
            }
        }

        /// Re-derive the token straight from the documented spec, independently of the function
        /// under test. If the function were a stub/constant, this would diverge.
        fn independent(inst: &Instance, dep: &Deployment, domain: &str) -> String {
            let mut h = TupleHash::v256(DOMAIN_VERIFICATION_TOKEN_CONTEXT);
            h.update(dep.app_id.as_ref());
            h.update(inst.provider.as_ref());
            h.update(inst.id.as_ref());
            h.update(domain.as_bytes());
            let mut out = [0u8; 32];
            h.finalize(&mut out);
            BASE64_STANDARD.encode(out)
        }

        #[test]
        fn not_empty_and_correct_shape() {
            let inst = instance(keys::alice::address(), 1);
            let dep = deployment("app1");
            let t = domain_verification_token(&inst, &dep, "example.com");

            assert!(
                !t.is_empty(),
                "token must not be empty (regression for the empty-buffer bug)"
            );
            assert_eq!(t.len(), 44, "base64 of 32 bytes is 44 chars");

            let raw = BASE64_STANDARD
                .decode(&t)
                .expect("token must be valid base64");
            assert_eq!(raw.len(), 32, "token must decode to a 256-bit digest");
            assert_ne!(raw, [0u8; 32], "token must not be all zeroes");
        }

        #[test]
        fn matches_independent_derivation_not_a_stub() {
            // Cross-check the real function against an independent TupleHash computation.
            let cases = [
                (
                    instance(keys::alice::address(), 1),
                    deployment("app1"),
                    "example.com",
                ),
                (
                    instance(keys::bob::address(), 42),
                    deployment("other"),
                    "sub.example.org",
                ),
                (
                    instance(keys::dave::address(), u64::MAX),
                    deployment("x"),
                    "",
                ),
            ];
            for (inst, dep, domain) in &cases {
                assert_eq!(
                    domain_verification_token(inst, dep, domain),
                    independent(inst, dep, domain),
                    "function output must equal the documented TupleHash derivation",
                );
            }
        }

        #[test]
        fn deterministic() {
            let inst = instance(keys::alice::address(), 7);
            let dep = deployment("app1");
            assert_eq!(
                domain_verification_token(&inst, &dep, "example.com"),
                domain_verification_token(&inst, &dep, "example.com"),
                "same inputs must yield the same token",
            );
        }

        #[test]
        fn bound_to_domain() {
            let inst = instance(keys::alice::address(), 1);
            let dep = deployment("app1");
            assert_ne!(
                domain_verification_token(&inst, &dep, "a.example.com"),
                domain_verification_token(&inst, &dep, "b.example.com"),
            );
        }

        #[test]
        fn bound_to_instance_id() {
            let dep = deployment("app1");
            assert_ne!(
                domain_verification_token(
                    &instance(keys::alice::address(), 1),
                    &dep,
                    "example.com"
                ),
                domain_verification_token(
                    &instance(keys::alice::address(), 2),
                    &dep,
                    "example.com"
                ),
            );
        }

        #[test]
        fn bound_to_provider() {
            let dep = deployment("app1");
            assert_ne!(
                domain_verification_token(
                    &instance(keys::alice::address(), 1),
                    &dep,
                    "example.com"
                ),
                domain_verification_token(&instance(keys::bob::address(), 1), &dep, "example.com"),
            );
        }

        #[test]
        fn bound_to_app_id() {
            let inst = instance(keys::alice::address(), 1);
            assert_ne!(
                domain_verification_token(&inst, &deployment("app1"), "example.com"),
                domain_verification_token(&inst, &deployment("app2"), "example.com"),
            );
        }

        #[test]
        fn no_prefix_or_substring_collisions() {
            // The hashed domain must be treated atomically: similar/related domain strings,
            // including a trailing-dot and sub/parent relationships, must all differ.
            let inst = instance(keys::alice::address(), 1);
            let dep = deployment("app1");
            let domains = [
                "example.com",
                "example.com.",
                "sub.example.com",
                "example.co",
                "xexample.com",
                "",
            ];
            let mut tokens: Vec<String> = domains
                .iter()
                .map(|d| domain_verification_token(&inst, &dep, d))
                .collect();
            tokens.sort();
            tokens.dedup();
            assert_eq!(
                tokens.len(),
                domains.len(),
                "all related domains must yield distinct tokens"
            );
        }
    }
}
