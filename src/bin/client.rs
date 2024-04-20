use env_logger::Env;
use log::info;
use soros::protocol_client::{ProtocolClient, ProtocolClientError};

fn main() -> Result<(), ProtocolClientError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();
    let mut protocol_client = ProtocolClient::new(1, "localhost", 8888)?;
    protocol_client.registry_create(10)?;
    protocol_client.registry_write(10)?;

    let err = protocol_client.registry_read(10);
    if let Err(ProtocolClientError::WaitError) = err {
        info!("Waiting...");
        protocol_client.registry_release(10)?
    }
    protocol_client.registry_expect_success()?;
    protocol_client.registry_release(10)?;

    let data_user = protocol_client.registry_read(10)?;
    info!("Data user of key: {:?}", data_user);
    protocol_client.registry_release(10)?;
    protocol_client.registry_delete(10)?;
    Ok(())
}
