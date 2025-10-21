// rofl-client/rs/src/lib.rs
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};

const DEFAULT_SOCKET: &str = "/run/rofl-appd.sock";

#[derive(Clone)]
pub struct RoflClient {
    socket_path: String,
}

impl RoflClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_socket_path(DEFAULT_SOCKET)
    }

    pub fn with_socket_path<P: AsRef<Path>>(
        socket_path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let socket_path = socket_path.as_ref().to_string_lossy().to_string();
        if !Path::new(&socket_path).exists() {
            return Err(format!("Socket not found at: {}", socket_path).into());
        }
        Ok(Self { socket_path })
    }

    // GET /rofl/v1/app/id
    pub async fn get_app_id(&self) -> Result<String, Box<dyn std::error::Error>> {
        let sock = self.socket_path.clone();
        let res = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let body = http_unix_request(&sock, "GET", "/rofl/v1/app/id", None, None)?;
            let s = String::from_utf8(body)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Ok(s.trim().to_string())
        })
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
        Ok(res)
    }

    // POST /rofl/v1/keys/generate
    pub async fn generate_key(
        &self,
        key_id: &str,
        kind: KeyKind,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let sock = self.socket_path.clone();
        let req = serde_json::to_vec(&KeyGenerationRequest {
            key_id: key_id.to_string(),
            kind: kind.to_string(),
        })?;
        let res = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let body = http_unix_request(
                &sock,
                "POST",
                "/rofl/v1/keys/generate",
                Some(&req),
                Some("application/json"),
            )?;
            let resp: KeyGenerationResponse = serde_json::from_slice(&body)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Ok(resp.key)
        })
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
        Ok(res)
    }

    // POST /rofl/v1/tx/sign-submit
    pub async fn sign_submit(
        &self,
        tx: Tx,
        encrypt: Option<bool>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let sock = self.socket_path.clone();
        let req = serde_json::to_vec(&SignSubmitRequest { tx, encrypt })?;
        let res = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
            let body = http_unix_request(
                &sock,
                "POST",
                "/rofl/v1/tx/sign-submit",
                Some(&req),
                Some("application/json"),
            )?;
            let resp: SignSubmitResponse = serde_json::from_slice(&body)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Ok(resp.data)
        })
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
        Ok(res)
    }

    /// Convenience helper for ETH-style call
    pub async fn sign_submit_eth(
        &self,
        gas_limit: u64,
        to: &str,
        value: u64,
        data_hex: &str,
        encrypt: Option<bool>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let eth = EthCall {
            gas_limit,
            to: to.to_string(),
            value,
            data: data_hex.to_string(),
        };
        self.sign_submit(Tx::Eth(eth), encrypt).await
    }
}

// Blocking HTTP-over-UDS request using std::os::unix::net::UnixStream
fn http_unix_request(
    socket_path: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    content_type: Option<&str>,
) -> std::io::Result<Vec<u8>> {
    use std::{
        io::{Error, ErrorKind, Read, Write},
        os::unix::net::UnixStream,
    };

    let mut stream = UnixStream::connect(socket_path)?;

    let mut req = Vec::new();
    req.extend_from_slice(format!("{method} {path} HTTP/1.1\r\n").as_bytes());
    req.extend_from_slice(b"Host: localhost\r\n");
    req.extend_from_slice(b"Connection: close\r\n");
    if let Some(ct) = content_type {
        req.extend_from_slice(format!("Content-Type: {ct}\r\n").as_bytes());
    }
    if let Some(b) = body {
        req.extend_from_slice(format!("Content-Length: {}\r\n", b.len()).as_bytes());
    }
    req.extend_from_slice(b"\r\n");
    if let Some(b) = body {
        req.extend_from_slice(b);
    }

    stream.write_all(&req)?;
    stream.flush()?;

    let mut resp = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        resp.extend_from_slice(&buf[..n]);
    }

    let header_end = resp
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                "Invalid HTTP response: no header/body delimiter",
            )
        })?;
    let (head, body_bytes) = resp.split_at(header_end + 4);

    let mut lines = head.split(|&b| b == b'\n');
    let status_line = lines
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Invalid HTTP response: empty"))?;
    let status_str = String::from_utf8(status_line.to_vec())
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    let code: u16 = status_str
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Invalid HTTP status line"))?
        .parse()
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

    if !(200..300).contains(&code) {
        let msg = String::from_utf8_lossy(body_bytes).to_string();
        return Err(Error::new(
            ErrorKind::Other,
            format!("HTTP {code} error: {msg}"),
        ));
    }

    Ok(body_bytes.to_vec())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyKind {
    Raw256,
    Raw384,
    Ed25519,
    Secp256k1,
}

impl std::fmt::Display for KeyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyKind::Raw256 => write!(f, "raw-256"),
            KeyKind::Raw384 => write!(f, "raw-384"),
            KeyKind::Ed25519 => write!(f, "ed25519"),
            KeyKind::Secp256k1 => write!(f, "secp256k1"),
        }
    }
}

#[derive(Debug, Serialize)]
struct KeyGenerationRequest {
    key_id: String,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct KeyGenerationResponse {
    key: String,
}

// -------------------- sign-submit types --------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Tx {
    #[serde(rename = "eth")]
    Eth(EthCall),
    #[serde(rename = "std")]
    Std(String), // CBOR-serialized hex-encoded Transaction
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthCall {
    pub gas_limit: u64,
    pub to: String,
    pub value: u64,
    pub data: String, // hex string without 0x prefix
}

#[derive(Debug, Serialize)]
struct SignSubmitRequest {
    pub tx: Tx,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypt: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SignSubmitResponse {
    pub data: String, // CBOR-serialized hex-encoded call result
}
