use std::{env, sync::Arc};
use rofl_appd::{Config, services::{kms::MockKmsService, metadata::InMemoryMetadataService}};

#[tokio::main]
async fn main() -> Result<(), rocket::Error> {
    let socket = env::args().nth(1).unwrap_or_else(|| "unix:/run/rofl-appd.sock".to_string());

    let config =
        Config {
            address: &socket,
            kms: Arc::new(MockKmsService),
            metadata: Arc::new(InMemoryMetadataService::default()),
        };

    println!("Starting minimal rofl-appd () at {}", socket);

    rofl_appd::start_mock(config).await
}