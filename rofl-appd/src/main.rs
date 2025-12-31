use rofl_appd::{
    services::{kms::MockKmsService, metadata::InMemoryMetadataService},
    Config,
};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), rocket::Error> {
    let socket = env::args()
        .nth(1)
        .unwrap_or_else(|| "unix:/run/rofl-appd.sock".to_string());
    let seed = env::args()
        .nth(2)
        .unwrap_or_else(|| "24b41929dc5bc3ec792f8792c7b7c32f".to_string());

    let config = Config {
        address: &socket,
        kms: Arc::new(MockKmsService),
        metadata: Arc::new(InMemoryMetadataService::default()),
    };

    println!("Starting minimal rofl-appd () at {}", socket);

    rofl_appd::start_local(config, None, seed.as_bytes()).await
}
