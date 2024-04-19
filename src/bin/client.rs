use env_logger::Env;
use log::info;
use soros::protocol_client::{ProtocolClient, ProtocolClientError};

fn main() -> Result<(), ProtocolClientError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();
    let mut protocol_client = ProtocolClient::new(1, "localhost", 8888)?;
    protocol_client.registry_create(10)?;
    Ok(())
}
