# rofl-client/rs/README.md
Rust client for the ROFL appd over a Unix domain socket.

- Default socket: `/run/rofl-appd.sock`
- Endpoints used:
  - `GET /rofl/v1/app/id`
  - `POST /rofl/v1/keys/generate`
  - `POST /rofl/v1/tx/sign-submit`

Quickstart:

```rust
use oasis_rofl_client::{KeyKind, RoflClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RoflClient::new()?;
    println!("app id: {}", client.get_app_id().await?);
    println!(
        "key: {}",
        client.generate_key("example", KeyKind::Ed25519).await?
    );
    Ok(())
}
```

Notes:
- Requires Unix socks. Windows is not supported unless using WSL.
- Methods are `async` and internally offload blocking UDS I/O via `tokio::task::spawn_blocking`.