mod domain;

use std::{collections::BTreeMap, ptr, sync::Weak, time::Duration};

use anyhow::{anyhow, Result};

use oasis_runtime_sdk::core::common::{crypto::x25519, logger::get_logger};
use oasis_runtime_sdk_rofl_market::types::{Deployment, Instance, InstanceId};
use rofl_app_core::prelude::*;
use rofl_proxy::{http, wireguard, HttpConfig, ProxyLabel};
use tokio::sync::Mutex;

use self::domain::{CancelVerificationsOnDrop, CustomDomainVerifier};
use crate::{
    config::ProxyConfig,
    proxy::domain::CustomDomainVerificationNotifier,
    types::{
        domain_verification_token, METADATA_KEY_PROXY_CUSTOM_DOMAINS, METADATA_KEY_PROXY_PROTOCOL,
        METADATA_KEY_PROXY_TIMEOUT_CONNECTION, METADATA_KEY_PROXY_TIMEOUT_RW,
        METADATA_VALUE_PROXY_PROTOCOL_V2,
    },
};

/// Maximum number of custom domains that can be configured at the same time by the same
/// deployment.
const MAX_CUSTOM_DOMAINS: usize = 3;

struct InstanceInfo {
    wireguard_pk: x25519::PublicKey,
    http_host: String,
    dst_address: String,
    dst_port: u16,
    proxy_protocol: bool,
    timeouts: http::Timeouts,
    /// Connection group shared by all mappings of the instance.
    connection_group: http::ConnectionGroup,
    /// Handle of the custom domain verifications queued for the current deployment.
    domain_verification_handle: Arc<CancelVerificationsOnDrop>,
}

/// A custom domain requested by a deployment.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CustomDomain {
    /// Domain as written in the deployment, from which the verification token is derived.
    provided: String,
    /// Canonical name of the domain, used for DNS verification, ownership and routing.
    canonical: String,
}

/// Current owner of each verified custom domain.
///
/// A domain can be verified by multiple instances (e.g. when moving it to a new instance), in
/// which case the instance that verified it last owns it. Domains are keyed by their canonical
/// name, the same as proxy routes, so differently written names refer to the same domain.
#[derive(Default)]
struct CustomDomainOwners(BTreeMap<String, InstanceId>);

impl CustomDomainOwners {
    /// Make the instance the owner of the domain, returning the previous owner (if different).
    fn claim(&mut self, domain: &str, id: InstanceId) -> Result<Option<InstanceId>> {
        let name = http::canonical_name(domain).ok_or_else(|| anyhow!("invalid domain name"))?;
        Ok(self.0.insert(name, id).filter(|previous| *previous != id))
    }

    /// Release all domains owned by the instance, returning their canonical names.
    fn release(&mut self, id: InstanceId) -> Vec<String> {
        let domains: Vec<String> = self
            .0
            .iter()
            .filter(|(_, owner)| **owner == id)
            .map(|(domain, _)| domain.clone())
            .collect();
        for domain in &domains {
            self.0.remove(domain);
        }
        domains
    }
}

/// Provisioned instances.
#[derive(Default)]
struct Instances {
    info: BTreeMap<InstanceId, InstanceInfo>,
    custom_domain_owners: CustomDomainOwners,
}

impl Instances {
    /// Remove an instance, returning its information and the custom domains that it owned.
    fn remove(&mut self, id: InstanceId) -> Option<(InstanceInfo, Vec<String>)> {
        let info = self.info.remove(&id)?;
        Some((info, self.custom_domain_owners.release(id)))
    }

    /// Make the instance the owner of a verified custom domain.
    ///
    /// The verification must have been queued for the instance's current deployment, as
    /// verifications for a previous deployment of the same instance may still complete.
    ///
    /// Returns the instance and the previous owner of the domain (if different). Fails if the
    /// verification is stale or the domain name is invalid.
    fn claim_custom_domain(
        &mut self,
        id: InstanceId,
        domain: &str,
        handle: &Weak<CancelVerificationsOnDrop>,
    ) -> Result<(&InstanceInfo, Option<InstanceId>)> {
        let info = self
            .info
            .get(&id)
            .ok_or_else(|| anyhow!("instance not provisioned"))?;
        // The weak reference keeps the allocation of a dropped handle alive, so its address
        // cannot be reused by the handle of a newer deployment.
        if !ptr::eq(
            handle.as_ptr(),
            Arc::as_ptr(&info.domain_verification_handle),
        ) {
            return Err(anyhow!("stale verification"));
        }
        let previous = self.custom_domain_owners.claim(domain, id)?;
        Ok((info, previous))
    }
}

/// Range of timeout values that deployments may request.
#[derive(Clone, Copy, Debug)]
struct TimeoutBounds {
    min: Duration,
    max: Duration,
}

impl TimeoutBounds {
    /// Create bounds from the proxy default and an optional maximum (in seconds).
    fn new(default: Duration, max: Option<u64>) -> Self {
        let max = max.map_or(default, Duration::from_secs);
        Self {
            min: default,
            max: max.max(default),
        }
    }

    fn clamp(&self, timeout: Duration) -> Duration {
        timeout.clamp(self.min, self.max)
    }
}

/// Policy for proxy timeouts that deployments may request.
#[derive(Clone, Copy, Debug)]
struct TimeoutPolicy {
    rw: TimeoutBounds,
    connection: TimeoutBounds,
}

impl TimeoutPolicy {
    fn new(cfg: &ProxyConfig, http_cfg: &http::Config) -> Self {
        Self {
            rw: TimeoutBounds::new(http_cfg.timeout_rw, cfg.max_timeout_rw),
            connection: TimeoutBounds::new(http_cfg.timeout_connection, cfg.max_timeout_connection),
        }
    }

    /// Proxy timeouts requested by the deployment, clamped to the policy bounds.
    ///
    /// Deployments can only raise timeouts as the proxy inside the instance enforces the exact
    /// per-port values.
    fn timeouts(&self, deployment: &Deployment) -> http::Timeouts {
        let requested = |key: &str, bounds: &TimeoutBounds| {
            deployment
                .metadata
                .get(key)
                .and_then(|value| value.parse::<u64>().ok())
                .map(|secs| bounds.clamp(Duration::from_secs(secs)))
        };

        http::Timeouts {
            rw: requested(METADATA_KEY_PROXY_TIMEOUT_RW, &self.rw),
            connection: requested(METADATA_KEY_PROXY_TIMEOUT_CONNECTION, &self.connection),
            ..Default::default()
        }
    }
}

/// Proxy for apps to expose services over.
pub struct Proxy {
    wireguard: wireguard::Hub,
    http: http::Proxy,
    domain: String,
    external_address: Option<String>,
    timeout_policy: TimeoutPolicy,
    max_instance_connections: Option<usize>,
    logger: slog::Logger,
    instances: Arc<Mutex<Instances>>,
    domain_verifier: CustomDomainVerifier,
}

impl Proxy {
    /// Create a new app proxy.
    pub fn new(
        cfg: &ProxyConfig,
        listen_port: u16,
        acme: rofl_proxy::http::tls::AcmeAccount,
    ) -> Result<Self> {
        let logger = get_logger("scheduler/proxy");

        // Instance hosts are subdomains of the proxy domain, so it must be a valid domain name.
        if http::canonical_name(&cfg.domain).is_none() {
            return Err(anyhow!("invalid proxy domain ({})", cfg.domain));
        }

        let wireguard = wireguard::Hub::new(wireguard::HubConfig {
            external_address: cfg.external_wireguard_address.clone(),
            external_port: wireguard::WG_DEFAULT_LISTEN_PORT,
        })?;

        let mut http_cfg = http::Config {
            listen_port,
            ..Default::default()
        };
        if let Some(timeout) = cfg.timeout_handshake {
            http_cfg.timeout_handshake = Duration::from_secs(timeout);
        }
        if let Some(timeout) = cfg.timeout_connect {
            http_cfg.timeout_connect = Duration::from_secs(timeout);
        }
        if let Some(timeout) = cfg.timeout_connection {
            http_cfg.timeout_connection = Duration::from_secs(timeout);
        }
        if let Some(timeout) = cfg.timeout_rw {
            http_cfg.timeout_rw = Duration::from_secs(timeout);
        }
        if let Some(max_connections) = cfg.max_connections {
            http_cfg.max_connections = max_connections as usize;
        }
        let timeout_policy = TimeoutPolicy::new(cfg, &http_cfg);
        let max_instance_connections = cfg
            .max_instance_connections
            .filter(|&max| max > 0)
            .map(|max| max as usize);
        let http = http::Proxy::new(http_cfg, acme)?;

        let instances = Arc::new(Mutex::new(Instances::default()));
        let notifier = Arc::new(ProxyDomainVerificationHandler::new(
            http.handle().clone(),
            instances.clone(),
            logger.clone(),
        ));

        Ok(Self {
            wireguard,
            http,
            domain: cfg.domain.clone(),
            external_address: cfg.external_proxy_address.clone(),
            timeout_policy,
            max_instance_connections,
            logger: logger.clone(),
            instances,
            domain_verifier: CustomDomainVerifier::new(8, notifier, logger),
        })
    }

    /// Start the app proxy services.
    pub fn start(&mut self) {
        slog::info!(self.logger, "starting proxy");

        self.http.start();
        self.domain_verifier.start();

        // Spawn task that periodically dumps Wireguard interface status.
        let logger = self.logger.clone();
        let wireguard = self.wireguard.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                match wireguard.current_status() {
                    Ok(status) => {
                        for (key, peer) in status.peers {
                            slog::info!(logger, "wireguard peer status";
                               "peer" => ?key,
                               "endpoint" => ?peer.endpoint,
                               "last_handshake" => ?peer.last_handshake,
                               "tx_bytes" => peer.tx_bytes,
                               "rx_bytes" => peer.rx_bytes,
                               "allowed_ips" => ?peer.allowed_ips,
                            );
                        }
                    }
                    Err(err) => {
                        slog::warn!(logger, "failed to get wireguard status"; "err" => ?err)
                    }
                }
            }
        });
    }

    /// Add a static HTTP proxy mapping.
    pub async fn add_static_mapping(&self, mapping: http::Mapping) -> Result<()> {
        self.http.add_mapping(mapping).await
    }

    /// Generate a fresh key pair and assign an IP address for the instance.
    ///
    /// Returns the proxy label that should be set on the deployed instance, encrypted to the
    /// app's SEK.
    pub async fn provision_instance(
        &self,
        instance: &Instance,
        deployment: &Deployment,
    ) -> Result<ProxyLabel> {
        let mut instances = self.instances.lock().await;

        // If the instance already exists, first tear down the existing instance.
        if let Some((info, custom_domains)) = instances.remove(instance.id) {
            self.teardown_instance(info, custom_domains).await?;
        }

        // Generate HTTP host.
        let short_id: u64 = instance.id.into();
        let http_host = format!("m{}.{}", short_id, self.domain);

        let wireguard = self.wireguard.provision_client()?;
        let dst_address = wireguard
            .address
            .split("/")
            .next()
            .ok_or(anyhow!("bad proxy listen address"))?
            .to_string();

        // Check if the deployment expects a PROXY protocol header.
        let proxy_protocol = deployment
            .metadata
            .get(METADATA_KEY_PROXY_PROTOCOL)
            .is_some_and(|v| v == METADATA_VALUE_PROXY_PROTOCOL_V2);

        let timeouts = self.timeout_policy.timeouts(deployment);
        let connection_group = http::ConnectionGroup::new(self.max_instance_connections);

        let result = self
            .http
            .add_mapping(http::Mapping {
                name: http_host.clone(),
                dst_address: dst_address.clone(),
                dst_port: 443,
                mode: http::Mode::ForwardOnly,
                proxy_protocol,
                timeouts,
                connection_group: Some(connection_group.clone()),
            })
            .await;
        if let Err(err) = result {
            // Not expected as the proxy domain is validated on startup.
            self.wireguard
                .deprovision_client(&wireguard.sk.public_key())?;
            return Err(err);
        }

        let domain_verification_handle = CancelVerificationsOnDrop::new();

        instances.info.insert(
            instance.id,
            InstanceInfo {
                wireguard_pk: wireguard.sk.public_key(),
                http_host: http_host.clone(),
                dst_address: dst_address.clone(),
                dst_port: 443,
                proxy_protocol,
                timeouts,
                connection_group,
                domain_verification_handle: domain_verification_handle.clone(),
            },
        );

        // Check if any custom domains have been configured and queue verification.
        let custom_domains = Self::extract_custom_domains(&self.logger, instance.id, deployment);
        for domain in &custom_domains {
            // Derive the token from the domain as written in the deployment (not its canonical
            // name) so that existing TXT records remain valid.
            let token = domain_verification_token(instance, deployment, &domain.provided);

            // Verify the canonical name, as that is the name that will be routed.
            if let Err(err) = self
                .domain_verifier
                .queue_verification(
                    instance.id,
                    &domain.canonical,
                    &token,
                    &domain_verification_handle,
                )
                .await
            {
                slog::error!(self.logger, "failed to queue domain verification";
                    "id" => ?instance.id,
                    "domain" => &domain.provided,
                    "err" => ?err
                );
            }
        }

        slog::info!(self.logger, "provisioned keys for instance";
            "id" => ?instance.id,
            "host" => &http_host,
            "address" => &dst_address,
            "timeouts" => ?timeouts,
        );

        let label = ProxyLabel {
            wireguard,
            http: HttpConfig {
                host: http_host,
                external_address: self.external_address.clone(),
                proxy_protocol,
            },
        };

        Ok(label)
    }

    /// Extract valid custom domains from deployment metadata.
    fn extract_custom_domains(
        logger: &slog::Logger,
        id: InstanceId,
        deployment: &Deployment,
    ) -> Vec<CustomDomain> {
        let empty = String::new();
        deployment
            .metadata
            .get(METADATA_KEY_PROXY_CUSTOM_DOMAINS)
            .unwrap_or(&empty)
            .split(' ')
            .filter(|s| !s.is_empty())
            .filter_map(|domain| {
                let Some(canonical) = http::canonical_name(domain) else {
                    slog::warn!(logger, "ignoring invalid custom domain";
                        "id" => ?id,
                        "domain" => domain,
                    );
                    return None;
                };
                Some(CustomDomain {
                    provided: domain.to_string(),
                    canonical,
                })
            })
            .take(MAX_CUSTOM_DOMAINS)
            .collect()
    }

    /// Deprovision the key pair associated with the given instance.
    pub async fn deprovision_instance(&self, id: InstanceId) -> Result<()> {
        let mut instances = self.instances.lock().await;
        if let Some((info, custom_domains)) = instances.remove(id) {
            self.teardown_instance(info, custom_domains).await?;

            slog::info!(self.logger, "deprovisioned keys for instance";
                "id" => ?id,
            );
        }
        Ok(())
    }

    /// Remove all proxy state of an instance and close its existing connections.
    ///
    /// Only the custom domains that the instance still owns are removed.
    async fn teardown_instance(
        &self,
        info: InstanceInfo,
        custom_domains: Vec<String>,
    ) -> Result<()> {
        // Remove mappings first so that new connections cannot join the closed group.
        self.http.remove_mapping(&info.http_host).await;
        for domain in &custom_domains {
            self.http.remove_mapping(domain).await;
        }
        info.connection_group.close();
        self.wireguard.deprovision_client(&info.wireguard_pk)?;
        Ok(())
    }
}

/// Handler for domain verification events.
struct ProxyDomainVerificationHandler {
    http: http::ProxyHandle,
    instances: Arc<Mutex<Instances>>,
    logger: slog::Logger,
}

impl ProxyDomainVerificationHandler {
    /// Creates a new instance of `ProxyDomainVerificationHandler`.
    pub fn new(
        http: http::ProxyHandle,
        instances: Arc<Mutex<Instances>>,
        logger: slog::Logger,
    ) -> Self {
        Self {
            http,
            instances,
            logger,
        }
    }
}

#[async_trait]
impl CustomDomainVerificationNotifier for ProxyDomainVerificationHandler {
    async fn verification_completed(
        &self,
        id: InstanceId,
        domain: &str,
        handle: &Weak<CancelVerificationsOnDrop>,
    ) {
        // Hold the lock until the mapping is added so that the instance cannot be redeployed in
        // the meantime.
        let mut instances = self.instances.lock().await;
        let (instance, previous) = match instances.claim_custom_domain(id, domain, handle) {
            Ok(claimed) => claimed,
            Err(err) => {
                slog::info!(self.logger, "ignoring custom domain verification";
                    "domain" => domain,
                    "id" => ?id,
                    "err" => ?err,
                );
                return;
            }
        };
        if let Some(previous) = previous {
            slog::info!(self.logger, "custom domain moved to another instance";
                "domain" => domain,
                "from" => ?previous,
                "to" => ?id,
            );
        }
        let mapping = http::Mapping {
            name: domain.to_string(),
            dst_address: instance.dst_address.clone(),
            dst_port: instance.dst_port,
            mode: http::Mode::ForwardOnly,
            proxy_protocol: instance.proxy_protocol,
            timeouts: instance.timeouts,
            connection_group: Some(instance.connection_group.clone()),
        };
        if let Err(err) = self.http.add_mapping(mapping).await {
            // Not expected as the domain name was validated when claiming it.
            slog::error!(self.logger, "failed to add custom domain mapping";
                "domain" => domain,
                "id" => ?id,
                "err" => ?err,
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn deployment(metadata: &[(&str, &str)]) -> Deployment {
        Deployment {
            metadata: metadata
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn test_timeout_policy() {
        let http_cfg = http::Config {
            timeout_rw: Duration::from_secs(30),
            timeout_connection: Duration::from_secs(45),
            ..Default::default()
        };
        let secs = |secs| Some(Duration::from_secs(secs));

        // Without configured maximums, deployments cannot raise timeouts.
        let policy = TimeoutPolicy::new(&ProxyConfig::default(), &http_cfg);
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "300"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "3600"),
        ]));
        assert_eq!(timeouts.rw, secs(30));
        assert_eq!(timeouts.connection, secs(45));

        let cfg = ProxyConfig {
            max_timeout_rw: Some(600),
            max_timeout_connection: Some(1800),
            ..Default::default()
        };
        let policy = TimeoutPolicy::new(&cfg, &http_cfg);

        // Missing or malformed requests should use the defaults.
        assert_eq!(policy.timeouts(&deployment(&[])), http::Timeouts::default());
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "invalid"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "-1"),
        ]));
        assert_eq!(timeouts, http::Timeouts::default());

        // Requests within the bounds should be honored.
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "300"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "900"),
        ]));
        assert_eq!(timeouts.rw, secs(300));
        assert_eq!(timeouts.connection, secs(900));
        assert_eq!(timeouts.connect, None);

        // Requests above the maximum should be clamped.
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "100000"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "100000"),
        ]));
        assert_eq!(timeouts.rw, secs(600));
        assert_eq!(timeouts.connection, secs(1800));

        // Requests cannot lower the timeouts below the defaults.
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "1"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "0"),
        ]));
        assert_eq!(timeouts.rw, secs(30));
        assert_eq!(timeouts.connection, secs(45));

        // Maximums below the defaults should not lower the defaults.
        let cfg = ProxyConfig {
            max_timeout_rw: Some(10),
            ..Default::default()
        };
        let policy = TimeoutPolicy::new(&cfg, &http_cfg);
        let timeouts = policy.timeouts(&deployment(&[(METADATA_KEY_PROXY_TIMEOUT_RW, "300")]));
        assert_eq!(timeouts.rw, secs(30));
    }

    fn instance_info(handle: &Arc<CancelVerificationsOnDrop>) -> InstanceInfo {
        InstanceInfo {
            wireguard_pk: x25519::PrivateKey::generate().public_key(),
            http_host: Default::default(),
            dst_address: Default::default(),
            dst_port: 443,
            proxy_protocol: false,
            timeouts: Default::default(),
            connection_group: http::ConnectionGroup::new(None),
            domain_verification_handle: handle.clone(),
        }
    }

    #[test]
    fn test_claim_custom_domain_stale_verification() {
        let a: InstanceId = 1.into();
        let b: InstanceId = 2.into();
        let domain = "app.example.org";
        let owner = |instances: &Instances| instances.custom_domain_owners.0.get(domain).copied();
        let mut instances = Instances::default();

        // Instance B currently owns the domain.
        let handle_b = CancelVerificationsOnDrop::new();
        instances.info.insert(b, instance_info(&handle_b));
        let claimed = instances.claim_custom_domain(b, domain, &Arc::downgrade(&handle_b));
        assert!(claimed.is_ok());

        // A verification is queued for the first deployment of instance A.
        let handle_a1 = CancelVerificationsOnDrop::new();
        let stale = Arc::downgrade(&handle_a1);
        instances.info.insert(a, instance_info(&handle_a1));

        // Instance A is redeployed, dropping the handle of the first deployment.
        instances.remove(a);
        drop(handle_a1);
        let handle_a2 = CancelVerificationsOnDrop::new();
        instances.info.insert(a, instance_info(&handle_a2));

        // The stale verification should not claim the domain.
        let result = instances.claim_custom_domain(a, domain, &stale);
        assert_eq!(result.err().unwrap().to_string(), "stale verification");
        assert_eq!(owner(&instances), Some(b));

        // A verification for the current deployment should claim it.
        let (_, previous) = instances
            .claim_custom_domain(a, domain, &Arc::downgrade(&handle_a2))
            .expect("verification should be current");
        assert_eq!(previous, Some(b));
        assert_eq!(owner(&instances), Some(a));

        // Verifications for removed instances should be ignored.
        let unknown: InstanceId = 3.into();
        assert!(instances
            .claim_custom_domain(unknown, domain, &Arc::downgrade(&handle_a2))
            .is_err());
        assert_eq!(owner(&instances), Some(a));
    }

    #[test]
    fn test_custom_domain_owners() {
        let a: InstanceId = 1.into();
        let b: InstanceId = 2.into();
        let mut owners = CustomDomainOwners::default();

        // Claiming an unowned domain or re-claiming an owned one should not report an owner.
        assert_eq!(owners.claim("a.example.com", a).unwrap(), None);
        assert_eq!(owners.claim("shared.example.com", a).unwrap(), None);
        assert_eq!(owners.claim("shared.example.com", a).unwrap(), None);
        assert_eq!(owners.claim("b.example.com", b).unwrap(), None);

        // Claiming a domain owned by another instance should move it.
        assert_eq!(owners.claim("shared.example.com", b).unwrap(), Some(a));

        // Releasing an instance should only return the domains that it still owns.
        assert_eq!(owners.release(a), vec!["a.example.com".to_string()]);
        assert!(owners.release(a).is_empty());
        assert_eq!(
            owners.release(b),
            vec![
                "b.example.com".to_string(),
                "shared.example.com".to_string()
            ]
        );

        // A released domain can be claimed again without reporting an owner.
        assert_eq!(owners.claim("shared.example.com", a).unwrap(), None);

        // Differently written names should refer to the same domain.
        assert_eq!(owners.claim("App.Example.org.", b).unwrap(), None);
        assert_eq!(owners.claim("app.example.org", a).unwrap(), Some(b));
        assert_eq!(owners.claim("APP.example.ORG", b).unwrap(), Some(a));
        assert_eq!(
            owners.release(a),
            vec!["shared.example.com".to_string()],
            "a domain moved under a different spelling should no longer be owned"
        );
        assert_eq!(owners.release(b), vec!["app.example.org".to_string()]);

        // Names with empty labels should be rejected without recording an owner.
        assert!(owners.claim("docs.example..", a).is_err());
        assert!(owners.claim("", a).is_err());
        assert!(owners.release(a).is_empty());

        // Released names should be usable as proxy route names unchanged.
        owners.claim("Docs.Example.", a).unwrap();
        for name in owners.release(a) {
            assert_eq!(http::canonical_name(&name), Some(name.clone()));
        }

        // Unicode and ASCII spellings of an internationalized domain are the same domain.
        assert_eq!(owners.claim("xn--bcher-kva.example.org", a).unwrap(), None);
        assert_eq!(owners.claim("Bücher.example.org", b).unwrap(), Some(a));
        assert!(owners.release(a).is_empty());
        assert_eq!(
            owners.release(b),
            vec!["xn--bcher-kva.example.org".to_string()]
        );
    }

    #[test]
    fn test_extract_custom_domains() {
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        let domains = [
            "Bücher.example.org",
            "",
            "docs.example..",
            "xn--bcher-kva.example.org",
            "App.Example.org.",
            "other.org",
        ];
        let deployment = deployment(&[(METADATA_KEY_PROXY_CUSTOM_DOMAINS, &domains.join(" "))]);
        let domain = |spelling: &str, canonical: &str| CustomDomain {
            provided: spelling.to_string(),
            canonical: canonical.to_string(),
        };

        // Domains should keep their spelling (as tokens are derived from it) next to their
        // canonical name, with invalid ones skipped without counting towards the limit.
        assert_eq!(
            Proxy::extract_custom_domains(&logger, Default::default(), &deployment),
            vec![
                domain("Bücher.example.org", "xn--bcher-kva.example.org"),
                domain("xn--bcher-kva.example.org", "xn--bcher-kva.example.org"),
                domain("App.Example.org.", "app.example.org"),
            ]
        );
    }
}
