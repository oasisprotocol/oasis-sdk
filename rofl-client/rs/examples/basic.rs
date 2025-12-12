// rofl-client/rs/examples/basic.rs
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
