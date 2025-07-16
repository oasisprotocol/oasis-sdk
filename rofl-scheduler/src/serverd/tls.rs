use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use base64::prelude::*;
use oasis_runtime_sdk::core::common::logger::get_logger;
use rcgen::{KeyPair, PublicKeyData, PKCS_ECDSA_P256_SHA256};
use rofl_app_core::prelude::*;
use rustls::{pki_types::pem::PemObject, sign::CertifiedKey};

/// Metadata key used to signal the used TLS public key. The value is `Base64(DER(pk))`.
const METADATA_KEY_TLS_PK: &str = "net.oasis.tls.pk";

static IDENTITY: OnceLock<Identity> = OnceLock::new();

/// Identity used in TLS connections.
pub struct Identity {
    key: KeyPair,
}

impl Identity {
    /// Return the global TLS identity instance. The global instance is initialized on
    /// first call to this method.
    pub fn global() -> Result<&'static Identity> {
        IDENTITY.get_or_try_init(Identity::generate)
    }

    /// Generate a new identity.
    fn generate() -> Result<Self> {
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        Ok(Self { key })
    }

    /// TLS key pair.
    pub fn key(&self) -> &KeyPair {
        &self.key
    }

    /// Metadata to be included in the instance metadata.
    pub fn metadata(&self) -> BTreeMap<String, String> {
        BTreeMap::from([(
            METADATA_KEY_TLS_PK.to_string(),
            BASE64_STANDARD.encode(self.key.subject_public_key_info()),
        )])
    }
}

/// Provisioner of TLS certificates via ACME/Let's Encrypt.
pub struct CertificateProvisioner {
    resolver: Arc<CertificateResolver>,
    domain: String,
    logger: slog::Logger,
}

impl CertificateProvisioner {
    /// Create a new certificate provisioner for the given domain.
    pub fn new(domain: &str) -> Self {
        Self {
            resolver: Arc::new(CertificateResolver::new()),
            domain: domain.to_owned(),
            logger: get_logger("serverd/cert-provisioner"),
        }
    }

    /// TLS server configuration using this certificate provisioner.
    pub fn server_config(&self) -> Arc<rustls::server::ServerConfig> {
        let mut cfg = rustls::server::ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(self.resolver.clone());
        cfg.alpn_protocols.push(H2_ALPN_PROTOCOL_ID.to_vec());
        cfg.alpn_protocols.push(HTTP11_ALPN_PROTOCOL_ID.to_vec());
        cfg.alpn_protocols.push(ACME_TLS_ALPN_PROTOCOL_ID.to_vec());
        Arc::new(cfg)
    }

    /// Starts the background provisioner task.
    pub async fn provision(self) {
        slog::info!(self.logger, "starting certificate provisioner task");

        // Initialize the ACME account.
        let mut acme = None;
        while acme.is_none() {
            // TODO: Support saving/loading the account information.
            let result = instant_acme::Account::builder()
                .unwrap()
                .create(
                    &instant_acme::NewAccount {
                        contact: &[],
                        terms_of_service_agreed: true,
                        only_return_existing: false,
                    },
                    instant_acme::LetsEncrypt::Production.url().to_owned(),
                    None,
                )
                .await;
            match result {
                Ok((acct, _)) => acme = Some(acct),
                Err(err) => {
                    slog::error!(self.logger, "failed to initialize ACME account"; "err" => ?err);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        let acme = acme.unwrap();
        slog::info!(self.logger, "ACME account initialized");

        loop {
            let delay = match self.provision_wait_time() {
                Ok(delay) => delay,
                Err(_) => Duration::ZERO,
            };
            slog::info!(self.logger, "waiting before provisioning certificate"; "delay" => ?delay);
            tokio::time::sleep(delay).await;

            let backoff = backoff::ExponentialBackoff::default();
            let _ = backoff::future::retry(backoff, async || {
                let result = self.provision_once(&acme).await;
                if let Err(ref err) = result {
                    slog::error!(self.logger, "failed to provision certificate"; "err" => ?err);
                }

                result.map_err(backoff::Error::transient)
            })
            .await;
        }
    }

    /// Compute the time we need to wait before trying to provision the certificate.
    fn provision_wait_time(&self) -> Result<Duration> {
        let certificate = match self.resolver.get_certificate() {
            Some(certificate) => certificate,
            None => return Ok(Duration::ZERO),
        };

        let (_, cert) = x509_parser::parse_x509_certificate(certificate.end_entity_cert()?)?;
        match cert.validity().time_to_expiration() {
            Some(ttl) => {
                let not_before = cert.validity().not_before;
                let not_after = cert.validity().not_after;
                match not_after - not_before {
                    Some(t) if ttl <= t / 3 => Ok(Duration::ZERO),
                    _ => Ok(ttl.try_into()?),
                }
            }
            None => Ok(Duration::ZERO),
        }
    }

    pub async fn provision_once(&self, acme: &instant_acme::Account) -> Result<()> {
        slog::info!(self.logger, "provisioning new certificate");

        // Create a new order.
        let mut order = acme
            .new_order(&instant_acme::NewOrder::new(&[
                instant_acme::Identifier::Dns(self.domain.clone()),
            ]))
            .await?;

        // Select TLS-ALPN authorization.
        let mut authorizations = order.authorizations();
        while let Some(result) = authorizations.next().await {
            let mut authz = result?;
            if !matches!(authz.status, instant_acme::AuthorizationStatus::Pending) {
                continue;
            }

            let mut challenge = authz
                .challenge(instant_acme::ChallengeType::TlsAlpn01)
                .ok_or_else(|| anyhow::anyhow!("no TLS-ALPN01 challenge found"))?;

            // Generate key.
            let mut params = rcgen::CertificateParams::new(vec![self.domain.clone()])?;
            params.custom_extensions = vec![rcgen::CustomExtension::new_acme_identifier(
                challenge.key_authorization().digest().as_ref(),
            )];
            let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
            let cert = params
                .self_signed(&key_pair)
                .context("failed to generate challenge certificate")?;
            let key_pair = rustls::pki_types::PrivateKeyDer::Pkcs8(
                rustls::pki_types::PrivatePkcs8KeyDer::from(key_pair.serialize_der()),
            );
            let key_pair = rustls::crypto::CryptoProvider::get_default()
                .ok_or(anyhow!("missing crypto provider"))?
                .key_provider
                .load_private_key(key_pair)
                .context("failed to load challenge private key")?;
            let certified_key = CertifiedKey::new(vec![cert.der().clone()], key_pair);
            self.resolver.set_challenge(Some(Arc::new(certified_key)));
            challenge.set_ready().await?;

            // Currently we only support a single identifier (domain).
            break;
        }

        // Exponentially back off until the order becomes ready or invalid.
        slog::info!(self.logger, "waiting for order to become ready");
        let status = order
            .poll_ready(&instant_acme::RetryPolicy::default())
            .await?;
        if status != instant_acme::OrderStatus::Ready {
            return Err(anyhow::anyhow!("unexpected order status: {status:?}"));
        }

        // Generate a CSR and finalize the order.
        slog::info!(self.logger, "generating a CSR and finalizing the order");
        let mut names = Vec::new();
        let mut identifiers = order.identifiers();
        while let Some(result) = identifiers.next().await {
            names.push(result?.to_string());
        }

        let mut params = rcgen::CertificateParams::new(names)?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        let key_pair = Identity::global()?.key();
        let csr = params.serialize_request(key_pair)?;
        order.finalize_csr(csr.der()).await?;
        let cert_chain_pem = order
            .poll_certificate(&instant_acme::RetryPolicy::default())
            .await?;

        let key_pair = rustls::pki_types::PrivateKeyDer::Pkcs8(
            rustls::pki_types::PrivatePkcs8KeyDer::from(key_pair.serialize_der()),
        );
        let certified_key = CertifiedKey::from_der(
            rustls::pki_types::CertificateDer::pem_slice_iter(cert_chain_pem.as_bytes())
                .collect::<Result<Vec<_>, _>>()?,
            key_pair,
            rustls::crypto::CryptoProvider::get_default()
                .ok_or(anyhow!("missing crypto provider"))?,
        )?;
        self.resolver.set_certificate(Some(Arc::new(certified_key)));

        slog::info!(self.logger, "certificate provisioned");

        Ok(())
    }
}

struct CertificateResolver {
    state: Mutex<CertificateResolverState>,
}

#[derive(Debug, Default)]
struct CertificateResolverState {
    challenge: Option<Arc<CertifiedKey>>,
    certificate: Option<Arc<CertifiedKey>>,
}

impl CertificateResolver {
    fn new() -> Self {
        Self {
            state: Mutex::new(CertificateResolverState::default()),
        }
    }

    fn set_challenge(&self, challenge: Option<Arc<CertifiedKey>>) {
        self.state.lock().unwrap().challenge = challenge;
    }

    fn get_certificate(&self) -> Option<Arc<CertifiedKey>> {
        self.state.lock().unwrap().certificate.clone()
    }

    fn set_certificate(&self, certificate: Option<Arc<CertifiedKey>>) {
        self.state.lock().unwrap().certificate = certificate;
    }
}

impl std::fmt::Debug for CertificateResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateResolver").finish()
    }
}

/// H2 ALPN protocol identifier.
const H2_ALPN_PROTOCOL_ID: &[u8] = b"h2";
/// HTTP/1.1 ALPN protocol identifier.
const HTTP11_ALPN_PROTOCOL_ID: &[u8] = b"http/1.1";
/// ACME-TLS ALPN protocol identifier as specified in RFC 8737.
const ACME_TLS_ALPN_PROTOCOL_ID: &[u8] = b"acme-tls/1";

/// Whether the given client hello contains exactly the ACME-TLS ALPN protocol.
fn is_acme_tls_alpn_protocol(client_hello: &rustls::server::ClientHello<'_>) -> bool {
    client_hello
        .alpn()
        .into_iter()
        .flatten()
        .eq([ACME_TLS_ALPN_PROTOCOL_ID])
}

impl rustls::server::ResolvesServerCert for CertificateResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        if is_acme_tls_alpn_protocol(&client_hello) {
            match client_hello.server_name() {
                Some(_) => self.state.lock().unwrap().challenge.clone(),
                None => None,
            }
        } else {
            self.state.lock().unwrap().certificate.clone()
        }
    }
}
