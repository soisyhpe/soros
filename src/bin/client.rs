use std::{thread, time::Duration};

use env_logger::Env;
use log::info;
use soros::{
    handle_wait,
    protocol_client::{ProtocolClient, ProtocolClientError},
};

fn usage_example() -> Result<(), ProtocolClientError> {
    let mut protocol_client = ProtocolClient::new("localhost", 8888)?;

    let mut _data = "my_data".to_string();
    let data_key = 1;

    protocol_client.registry_create(data_key)?;
    protocol_client.registry_create(data_key)?;
    protocol_client.registry_write_sync(data_key)?;

    info!("Write of the data with key {}", data_key);
    _data = "modified_data".to_string();

    protocol_client.registry_release(data_key)?;

    let _holder = protocol_client.registry_read(data_key)?;

    // tODO: missing peer to peer implementation to get the data content
    info!("Read of the data with key {}", data_key);

    protocol_client.registry_release(data_key)?;
    protocol_client.registry_delete(data_key)?;

    Ok(())
}

fn advanced_usage() -> Result<(), ProtocolClientError> {
    let mut protocol_client = ProtocolClient::new("localhost", 8888)?;
    let data_key = 2;

    protocol_client.registry_create(data_key)?;
    handle_wait!(protocol_client.registry_write(data_key), {
        protocol_client.registry_expect_success(data_key)?;
    });

    handle_wait!(protocol_client.registry_read(data_key), {
        info!("Waiting for {}...", data_key);
        thread::sleep(Duration::from_secs(2));
        protocol_client.registry_release(data_key)?
    });

    protocol_client.registry_expect_holder(data_key)?;
    protocol_client.registry_release(data_key)?;

    let data_user = protocol_client.registry_read_sync(data_key)?;
    info!("Data user of key: {:?}", data_user);

    protocol_client.registry_release(data_key)?;
    protocol_client.registry_delete(data_key)?;

    Ok(())
}

fn main() -> Result<(), ProtocolClientError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();

    let handles =
        vec![thread::spawn(usage_example), thread::spawn(advanced_usage)];

    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}
