use std::{thread, time::Duration};

use env_logger::Env;
use log::info;
use soros::protocol_client::{ProtocolClient, ProtocolClientError};

fn main() -> Result<(), ProtocolClientError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();
    let mut protocol_client = ProtocolClient::new("localhost", 8888)?;

    let _ = protocol_client.registry_create(10);

    if let Err(ProtocolClientError::WaitError(10)) =
        protocol_client.registry_write(10)
    {
        protocol_client.registry_expect_success(10)?;
    }

    let err = protocol_client.registry_read(10);
    if let Err(ProtocolClientError::WaitError(10)) = err {
        info!("Waiting...");
        thread::sleep(Duration::from_secs(5));
        protocol_client.registry_release(10)?
    }
    protocol_client.registry_expect_holder(10)?;
    protocol_client.registry_release(10)?;

    let data_user = protocol_client.registry_read_sync(10)?;
    info!("Data user of key: {:?}", data_user);
    protocol_client.registry_release(10)?;

    // protocol_client.registry_delete(10)?;

    Ok(())
}
