use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use base64::prelude::*;
use oasis_runtime_sdk::core::common::logger::get_logger;
use p256::pkcs8::EncodePrivateKey;
use rcgen::{KeyPair, PublicKeyData, PKCS_ECDSA_P256_SHA256};
use rustls::{
    pki_types::{pem::PemObject, PrivateKeyDer, PrivatePkcs8KeyDer},
    sign::CertifiedKey,
};
use tokio::{sync::mpsc, task::JoinHandle};
use zeroize::Zeroizing;

/// Re-export instant_acme types for convenience.
pub use instant_acme::{Account as AcmeAccount, LetsEncrypt};

/// Metadata key used to signal the used TLS public key. The value is `Base64(DER(pk))`.
const METADATA_KEY_TLS_PK: &str = "net.oasis.tls.pk";

/// Location of the persistent TLS private key file.
const PERSISTENT_TLS_KEY_PATH: &str = "/storage/tls/identity";
/// Location of the persistent TLS certificates directory.
const PERSISTENT_TLS_CERTS_DIR: &str = "/storage/tls/certs";
/// Rotate persistent TLS identity and certificates after this time.
const PERSISTENT_TLS_ROTATE_AFTER_SECS: u64 = 7 * 24 * 3600; // 1 week

/// KMS key identifier for the ACME account key.
const KMS_ACME_KEY_ID: &str = "rofl-proxy/tls: acme account key v1";

static IDENTITY: OnceLock<Identity> = OnceLock::new();

/// Identity used in TLS connections.
pub struct Identity {
    key: KeyPair,
}

impl Identity {
    /// Initialize the global TLS identity.
    pub fn init() -> Result<()> {
        IDENTITY.get_or_try_init(Identity::load_or_generate)?;
        Ok(())
    }

    /// Return the global TLS identity instance iff initialized.
    ///
    /// If the identity has not yet been initialized it returns `None`.
    pub fn global() -> Option<&'static Identity> {
        IDENTITY.get()
    }

    /// Load or generate a new identity.
    fn load_or_generate() -> Result<Self> {
        let key = match read_to_string_with_expiry(
            PERSISTENT_TLS_KEY_PATH,
            PERSISTENT_TLS_ROTATE_AFTER_SECS,
        ) {
            Ok(data) => {
                let data = Zeroizing::new(data);
                KeyPair::from_pem(&data)?
            }
            Err(_) => {
                let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
                let pem = Zeroizing::new(key.serialize_pem());
                // Ignore errors while persisting the key as these are not fatal.
                let _ = write_from_string(PERSISTENT_TLS_KEY_PATH, &pem);
                key
            }
        };
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

/// Internal provisioner command.
enum Command {
    AddDomain(String),
    RemoveDomain(String),
}

#[derive(Clone)]
pub struct CertificateProvisionerHandle {
    cmd_tx: mpsc::Sender<Command>,
}

impl CertificateProvisionerHandle {
    /// Add a new domain to this TLS provisioner.
    pub async fn add_domain(&self, sni: &str) {
        let _ = self.cmd_tx.send(Command::AddDomain(sni.to_string())).await;
    }

    /// Remove a domain from this TLS provisioner.
    pub async fn remove_domain(&self, sni: &str) {
        let _ = self
            .cmd_tx
            .send(Command::RemoveDomain(sni.to_string()))
            .await;
    }
}

/// Raw ACME account key material that will be zeroized on drop.
#[derive(Debug, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct RawAcmeKey(pub Vec<u8>);

impl AsRef<[u8]> for RawAcmeKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Initialize an ACME account from a key source (typically KMS-derived).
pub async fn init_acme_account<F, Fut>(
    derive_raw_key: F,
    directory_url: &str,
) -> Result<AcmeAccount>
where
    for<'a> F: Fn(&'a [u8]) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<RawAcmeKey>> + Send,
{
    let logger = get_logger("acme-account");

    let acme_key = derive_raw_key(KMS_ACME_KEY_ID.as_bytes()).await?;

    // Back off to recover from Let's Encrypt authorization-failure limits.
    // Limit: 10 new accounts per IP every 3 hours, refiling 1 slot every ~18 minutes.
    // So we make the final interval >18 min.
    //
    // Note however, even with backoff, if multiple apps on the same host attempt
    // to provision an ACME account concurrently, they may all hit this limit.
    // Because rate-limited failures also count toward this limit this could create a never
    // ending backoff situation. We try to mitigate this by using 1 hour max backoff and
    // deterministic keys to avoid re-provisioning new accounts for apps on restarts.
    let backoff = backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_secs(30))
        .with_multiplier(2.0)
        .with_randomization_factor(0.1)
        .with_max_interval(Duration::from_secs(60 * 60))
        .with_max_elapsed_time(Some(Duration::from_secs(2 * 60 * 60)))
        .build();
    backoff::future::retry(backoff, || async {
        let result = try_init_acme_account(acme_key.as_ref(), directory_url).await;
        if let Err(ref err) = result {
            slog::error!(logger, "failed to create ACME account"; "err" => ?err);
        }
        result.map_err(backoff::Error::transient)
    })
    .await
}

/// Try to create an ACME account from raw key bytes.
async fn try_init_acme_account(acme_key: &[u8], directory_url: &str) -> Result<AcmeAccount> {
    let (acme_key, private_key_der) =
        raw_to_acme_key_pair(acme_key).context("failed to convert raw key into ACME key pair")?;

    let (acc, _creds) = instant_acme::Account::builder()?
        .create_from_key((acme_key, private_key_der), directory_url.to_string())
        .await?;
    Ok(acc)
}

/// Convert raw 32-byte entropy from KMS into a P-256 key pair for ACME.
fn raw_to_acme_key_pair(raw_key: &[u8]) -> Result<(instant_acme::Key, PrivateKeyDer<'static>)> {
    let secret_key = p256::SecretKey::from_slice(raw_key)
        .map_err(|e| anyhow!("failed to create P-256 secret key: {e}"))?;

    let pkcs8 = Zeroizing::new(
        secret_key
            .to_pkcs8_der()
            .map_err(|e| anyhow!("failed to serialize key to PKCS#8: {e}"))?
            .as_bytes()
            .to_vec(),
    );
    let acme_key =
        instant_acme::Key::from_pkcs8_der(PrivatePkcs8KeyDer::from(pkcs8.clone().to_vec()))?;
    let private_key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8.clone().to_vec()));
    Ok((acme_key, private_key_der))
}

/// Provisioner of TLS certificates via ACME/Let's Encrypt.
pub struct CertificateProvisioner {
    resolver: Arc<CertificateResolver>,
    logger: slog::Logger,
    cmd_rx: Option<mpsc::Receiver<Command>>,
    handle: CertificateProvisionerHandle,
    acme: AcmeAccount,
}

impl CertificateProvisioner {
    /// Create a new certificate provisioner.
    pub fn new(acme: AcmeAccount) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(16);

        Self {
            resolver: Arc::new(CertificateResolver::new()),
            logger: get_logger("serverd/cert-provisioner"),
            cmd_rx: Some(cmd_rx),
            handle: CertificateProvisionerHandle { cmd_tx },
            acme,
        }
    }

    /// TLS server configuration using this certificate provisioner.
    pub fn server_config(&self, alpn_h2: bool) -> Arc<rustls::server::ServerConfig> {
        let mut cfg = rustls::server::ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(self.resolver.clone());
        if alpn_h2 {
            cfg.alpn_protocols.push(H2_ALPN_PROTOCOL_ID.to_vec());
        }
        cfg.alpn_protocols.push(HTTP11_ALPN_PROTOCOL_ID.to_vec());
        cfg.alpn_protocols.push(ACME_TLS_ALPN_PROTOCOL_ID.to_vec());
        Arc::new(cfg)
    }

    /// Certificate provisioner handle.
    pub fn handle(&self) -> &CertificateProvisionerHandle {
        &self.handle
    }

    /// Start the TLS provisioner.
    pub fn start(mut self) {
        if let Some(cmd_rx) = self.cmd_rx.take() {
            let this = Arc::new(self);
            tokio::spawn(this.manager(cmd_rx));
        }
    }

    async fn manager(self: Arc<Self>, mut cmd_rx: mpsc::Receiver<Command>) {
        slog::info!(self.logger, "starting certificate provisioner task");

        let mut domains: HashMap<String, JoinHandle<()>> = HashMap::new();

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                Command::AddDomain(sni) => {
                    if domains.contains_key(&sni) {
                        continue;
                    }

                    slog::info!(self.logger, "adding new domain"; "sni" => &sni);

                    let handle =
                        tokio::spawn(self.clone().worker(sni.to_string(), self.acme.clone()));
                    domains.insert(sni.to_string(), handle);
                }
                Command::RemoveDomain(sni) => {
                    if let Some(handle) = domains.remove(&sni) {
                        handle.abort();
                        slog::info!(self.logger, "removing domain"; "sni" => &sni);
                    }
                }
            }
        }
    }

    async fn worker(self: Arc<Self>, sni: String, acme: AcmeAccount) {
        let sni = &sni;

        // First attempt to load existing certificate from persistent storage.
        if let Err(err) = self.try_load_certificate(sni) {
            slog::info!(self.logger, "failed to load existing certificate";
                "err" => ?err,
                "sni" => sni,
            );
        }

        loop {
            let delay = match self.provision_wait_time(sni) {
                Ok(delay) => delay,
                Err(_) => Duration::ZERO,
            };
            slog::info!(self.logger, "waiting before provisioning certificate";
                "sni" => sni,
                "delay" => ?delay,
            );
            tokio::time::sleep(delay).await;

            // Back off to recover from Let's Encrypt authorization-failure limits.
            // Limit: 5 failed authorizations per identifier per account per hour,
            // refilling 1 slot every ~12 minutes. Make the final interval >12 min.
            // Rate-limited failures also count toward this limit.
            // See: https://letsencrypt.org/docs/rate-limits/#authorization-failures-per-identifier-per-account
            let backoff = backoff::ExponentialBackoffBuilder::new()
                .with_initial_interval(Duration::from_secs(30))
                .with_multiplier(2.0)
                .with_randomization_factor(0.1)
                .with_max_interval(Duration::from_secs(13 * 60))
                .with_max_elapsed_time(Some(Duration::from_secs(45 * 60)))
                .build();
            let _ = backoff::future::retry(backoff, async || {
                let result = self.provision_once(sni, &acme).await;
                if let Err(ref err) = result {
                    slog::error!(self.logger, "failed to provision certificate";
                        "err" => ?err,
                        "sni" => sni,
                    );
                }

                result.map_err(backoff::Error::transient)
            })
            .await;
        }
    }

    /// Try to load previously persisted certificate.
    fn try_load_certificate(&self, sni: &str) -> Result<()> {
        let data = read_to_string_with_expiry(
            Path::new(PERSISTENT_TLS_CERTS_DIR).join(sni),
            PERSISTENT_TLS_ROTATE_AFTER_SECS,
        )?;
        let data = Zeroizing::new(data);

        let key_pair = Identity::global()
            .ok_or(anyhow!("identity not initialized"))?
            .key();
        let key_pair = rustls::pki_types::PrivateKeyDer::Pkcs8(
            rustls::pki_types::PrivatePkcs8KeyDer::from(key_pair.serialize_der()),
        );

        let certified_key = CertifiedKey::from_der(
            rustls::pki_types::CertificateDer::pem_slice_iter(data.as_bytes())
                .collect::<Result<Vec<_>, _>>()?,
            key_pair,
            rustls::crypto::CryptoProvider::get_default()
                .ok_or(anyhow!("missing crypto provider"))?,
        )?;
        self.resolver
            .set_certificate(sni, Some(Arc::new(certified_key)));
        Ok(())
    }

    /// Compute the time we need to wait before trying to provision the certificate.
    fn provision_wait_time(&self, sni: &str) -> Result<Duration> {
        let certificate = match self.resolver.get_certificate(sni) {
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
                    Some(t) => Ok(ttl.checked_sub(t / 3).unwrap_or_default().try_into()?),
                    _ => Ok(ttl.try_into()?),
                }
            }
            None => Ok(Duration::ZERO),
        }
    }

    pub async fn provision_once(&self, sni: &str, acme: &AcmeAccount) -> Result<()> {
        slog::info!(self.logger, "provisioning new certificate"; "sni" => sni);

        // Create a new order.
        let mut order = acme
            .new_order(&instant_acme::NewOrder::new(&[
                instant_acme::Identifier::Dns(sni.to_string()),
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
            let mut params = rcgen::CertificateParams::new(vec![sni.to_string()])?;
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
            self.resolver
                .set_challenge(sni, Some(Arc::new(certified_key)));
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
        let key_pair = Identity::global()
            .ok_or(anyhow!("identity not initialized"))?
            .key();
        let csr = params.serialize_request(key_pair)?;
        order.finalize_csr(csr.der()).await?;
        let cert_chain_pem = order
            .poll_certificate(&instant_acme::RetryPolicy::default())
            .await?;

        // Persist new certificate.
        if let Err(err) = write_from_string(
            Path::new(PERSISTENT_TLS_CERTS_DIR).join(sni),
            &cert_chain_pem,
        ) {
            slog::error!(self.logger, "failed to persist certificate";
                "err" => ?err,
                "sni" => sni,
            );
        }

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
        self.resolver
            .set_certificate(sni, Some(Arc::new(certified_key)));

        slog::info!(self.logger, "certificate provisioned");

        Ok(())
    }
}

struct CertificateResolver {
    state: Mutex<CertificateResolverState>,
}

#[derive(Debug, Default)]
struct CertificateResolverState {
    challenges: HashMap<String, Arc<CertifiedKey>>,
    certificates: HashMap<String, Arc<CertifiedKey>>,
}

impl CertificateResolver {
    fn new() -> Self {
        Self {
            state: Mutex::new(CertificateResolverState::default()),
        }
    }

    fn set_challenge(&self, sni: &str, challenge: Option<Arc<CertifiedKey>>) {
        let mut state = self.state.lock().unwrap();

        match challenge {
            Some(challenge) => {
                state.challenges.insert(sni.to_string(), challenge);
            }
            None => {
                state.challenges.remove(sni);
            }
        }
    }

    fn get_challenge(&self, sni: &str) -> Option<Arc<CertifiedKey>> {
        self.state.lock().unwrap().challenges.get(sni).cloned()
    }

    fn get_certificate(&self, sni: &str) -> Option<Arc<CertifiedKey>> {
        self.state.lock().unwrap().certificates.get(sni).cloned()
    }

    fn set_certificate(&self, sni: &str, certificate: Option<Arc<CertifiedKey>>) {
        let mut state = self.state.lock().unwrap();

        match certificate {
            Some(certificate) => {
                state.challenges.remove(sni);
                state.certificates.insert(sni.to_string(), certificate);
            }
            None => {
                state.certificates.remove(sni);
            }
        }
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
pub(super) const ACME_TLS_ALPN_PROTOCOL_ID: &[u8] = b"acme-tls/1";

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
        let sni = client_hello.server_name()?;
        if is_acme_tls_alpn_protocol(&client_hello) {
            self.get_challenge(sni)
        } else {
            self.get_certificate(sni)
        }
    }
}

fn read_to_string_with_expiry<P>(path: P, expiry: u64) -> Result<String>
where
    P: AsRef<Path>,
{
    let metadata = fs::metadata(&path)?;
    let age = metadata.created()?.elapsed()?;
    if age > Duration::from_secs(expiry) {
        return Err(anyhow!("existing file expired"));
    }
    Ok(fs::read_to_string(path)?)
}

fn write_from_string<P>(path: P, data: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = fs::OpenOptions::new()
        .mode(0o600)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    file.write_all(data.as_ref())?;
    file.sync_all()?;
    if let Some(dir) = path.parent() {
        fs::File::open(dir)?.sync_all()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rofl_appd::services::kms::{GenerateRequest, KeyKind, KmsService, MockKmsService};

    #[test]
    fn test_raw_to_acme_key_pair() {
        let kms = MockKmsService;

        // Generate raw 32-byte key from the Mock KMS.
        let response = tokio_test::block_on(kms.generate(&GenerateRequest {
            key_id: KMS_ACME_KEY_ID,
            kind: KeyKind::Raw256,
        }))
        .expect("failed to generate key");

        // Convert raw key to ACME key pair.
        let (_acme_key, private_key_der) = raw_to_acme_key_pair(&response.key)
            .expect("failed to convert raw key to ACME key pair");

        // Verify we got a valid PKCS8 private key.
        assert!(
            matches!(private_key_der, rustls::pki_types::PrivateKeyDer::Pkcs8(_)),
            "private key should be PKCS8 format"
        );
    }

    /// This test requires network access to Let's Encrypt staging servers.
    /// Note: This test relies on network access and let's encrypt staging servers.
    #[tokio::test]
    async fn test_acme_account_creation() {
        use rand::Rng;
        let kms = Arc::new(MockKmsService);

        let timeout = Duration::from_secs(30);
        let acme = tokio::time::timeout(
            timeout,
            init_acme_account(
                move |_key_id: &[u8]| {
                    let kms = kms.clone();
                    async move {
                        // Use random key ID for test to avoid conflicts with previous test runs.
                        let random_bytes: [u8; 32] = rand::thread_rng().gen();
                        let key_id = format!("test-acme-account-{}", hex::encode(&random_bytes));
                        let response = kms
                            .generate(&GenerateRequest {
                                key_id: &key_id,
                                kind: KeyKind::Raw256,
                            })
                            .await
                            .map_err(|e| anyhow!("failed to generate key: {}", e))?;
                        Ok(RawAcmeKey(response.key.clone()))
                    }
                },
                LetsEncrypt::Staging.url(),
            ),
        )
        .await
        .expect("timed out creating ACME account")
        .expect("failed to create ACME account");

        println!("ACME account created: {:?}", acme.id());
    }
}
