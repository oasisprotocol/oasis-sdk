# rofl-client/rs/README.md
Rust client for the ROFL appd over a Unix domain socket.

- Default socket: `/run/rofl-appd.sock`
- Endpoints used:
  - `GET /rofl/v1/app/id`
  - `POST /rofl/v1/keys/generate`
  - `POST /rofl/v1/tx/sign-submit`

Quickstart:

```rust
use oasis_rofl_client::{RoflClient, KeyKind, Tx};

# #[tokio::main]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
let client = RoflClient::new()?; // or with_socket_path("/custom.sock")

let app_id = client.get_app_id().await?;
let key = client.generate_key("my-key", KeyKind::Secp256k1).await?;

// ETH-style call
let result = client
    .sign_submit_eth(21_000, "0xdeadbeef...", 0, "a9059cbb...", None)
    .await?;
# Ok(())
# }
```

Notes:
- Requires Unix (UDS). Windows is not supported unless using WSL.
- Methods are `async` and internally offload blocking UDS I/O via `tokio::task::spawn_blocking`.