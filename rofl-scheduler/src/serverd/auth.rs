use std::sync::{Arc, LazyLock};

use anyhow::{anyhow, Context};
use axum::{extract, http, response, RequestPartsExt};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use chrono::{prelude::*, Duration};
use rand::RngCore;
use rustc_hex::FromHex;

use oasis_runtime_sdk::{
    crypto::signature::{self, PublicKey},
    types::address::{Address, SignatureAddressSpec},
};

use super::{error::Error, State};

/// Global JWT keys instance.
static JWT_KEYS: LazyLock<Keys> = LazyLock::new(Keys::generate);

/// JWT keys.
struct Keys {
    encoding: jsonwebtoken::EncodingKey,
    decoding: jsonwebtoken::DecodingKey,
}

impl Keys {
    /// Generate a new pair of JWT keys.
    fn generate() -> Self {
        let mut key = [0; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);

        Self {
            encoding: jsonwebtoken::EncodingKey::from_secret(&key),
            decoding: jsonwebtoken::DecodingKey::from_secret(&key),
        }
    }
}

/// Claims issued after successful authentication.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub address: String,
    exp: u64,
}

impl<S> extract::FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Extract the token from the authorization header.
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| Error::BadAuthToken)?;

        // Validate the token and decode the user data.
        let token_data = jsonwebtoken::decode::<Claims>(
            bearer.token(),
            &JWT_KEYS.decoding,
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|_| Error::BadAuthToken)?;

        Ok(token_data.claims)
    }
}

/// Login request.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "method", content = "data")]
pub enum AuthLoginRequest {
    #[serde(rename = "siwe")]
    Siwe { message: String, signature: String },

    #[serde(rename = "std")]
    Std { body: String, signature: String },
}

impl AuthLoginRequest {
    /// Verify the login request and return the authenticated claims.
    pub async fn verify(&self, domain: &str, provider: Address) -> Result<Claims, anyhow::Error> {
        let address = match self {
            Self::Siwe { message, signature } => {
                let message: siwe::Message = message.parse().context("malformed SIWE message")?;
                let signature: Vec<u8> =
                    signature.from_hex().context("malformed SIWE signature")?;
                let signature: [u8; 65] = signature
                    .try_into()
                    .map_err(|_| anyhow!("malformed SIWE signature"))?;

                if !message.valid_now() {
                    return Err(anyhow!("message is not yet valid or has expired"));
                }

                let expected_statement = format!(
                    "Authenticate to ROFL provider {} to manage your machines via API at {}.",
                    provider.to_bech32(),
                    domain,
                );
                if message.statement != Some(expected_statement) {
                    return Err(anyhow!("message does not have the expected statement"));
                }

                let verification_opts = siwe::VerificationOpts {
                    // We currently allow any origin domain.
                    ..Default::default()
                };
                message.verify(&signature, &verification_opts).await?;

                Address::from_eth(&message.address)
            }
            Self::Std { body, signature } => {
                // Decode Base64-encoded body.
                let raw_body = BASE64_STANDARD
                    .decode(body)
                    .context("malformed authentication statement body")?;
                let body: StdAuthBody = cbor::from_slice(&raw_body)
                    .context("malformed authentication statement body")?;
                // Validate body.
                body.validate(domain, provider)?;

                // Decode Base64-encoded signature.
                let signature = BASE64_STANDARD
                    .decode(signature)
                    .context("malformed signature")?
                    .into();
                // Verify signature.
                let ctx = signature::context::get_chain_context_for(STD_AUTH_CONTEXT_BASE);
                body.signer.verify(&ctx, &raw_body, &signature)?;

                Address::from_sigspec(
                    &SignatureAddressSpec::try_from_pk(&body.signer)
                        .ok_or(anyhow!("unsupported public key scheme"))?,
                )
            }
        };

        Ok(Claims {
            address: address.to_bech32(),
            ..Default::default()
        })
    }
}

/// Signature context used for standard authentication.
pub const STD_AUTH_CONTEXT_BASE: &[u8] = b"rofl-scheduler/auth: v1";

/// Standard authentication statement body.
#[derive(Debug, Clone, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct StdAuthBody {
    pub v: u16,
    pub domain: String,
    pub provider: Address,
    pub signer: PublicKey,
    pub nonce: String,
    pub not_before: u64,
    pub not_after: u64,
}

impl StdAuthBody {
    /// Validate the authentication statement.
    pub fn validate(&self, domain: &str, provider: Address) -> Result<(), anyhow::Error> {
        if self.v != 1 {
            return Err(anyhow!("unsupported version"));
        }
        if self.domain != domain {
            return Err(anyhow!(
                "mismatched domain (expected: {} got: {})",
                domain,
                self.domain
            ));
        }
        if self.provider != provider {
            return Err(anyhow!(
                "mismatched provider (expected: {} got: {})",
                provider,
                self.provider
            ));
        }
        let now = Utc::now().timestamp() as u64;
        if now < self.not_before {
            return Err(anyhow!("statement not yet valid"));
        }
        if now > self.not_after {
            return Err(anyhow!("statement has expired"));
        }
        Ok(())
    }
}

/// Response from the login handler.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct AuthLoginResponse {
    /// Issued JWT.
    pub token: String,
    /// Expiration timestamp.
    pub expiry: u64,
}

/// Login handler.
pub async fn login(
    extract::State(state): extract::State<Arc<State>>,
    extract::Json(request): extract::Json<AuthLoginRequest>,
) -> Result<response::Json<AuthLoginResponse>, Error> {
    let mut claims = request
        .verify(&state.domain, state.provider)
        .await
        .map_err(|_| Error::Forbidden)?;

    let expiry = Utc::now() + Duration::seconds(state.token_lifetime as i64);
    claims.exp = expiry.timestamp() as u64;

    // Issue the JWT.
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    let token = jsonwebtoken::encode(&header, &claims, &JWT_KEYS.encoding)?;

    Ok(response::Json(AuthLoginResponse {
        token,
        expiry: claims.exp,
    }))
}
