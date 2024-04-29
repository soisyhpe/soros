use std::{env, thread};
use std::sync::Arc;

use env_logger::Env;
use log::{error, info, warn};
use soros::{
    handle_wait_error,
    protocol_client::{ProtocolClient, ProtocolClientError},
};
use soros::peer2peer_server::{DataStore, P2PServer};

fn create_protocol_client() -> Result<ProtocolClient, ProtocolClientError> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Usage: {} <hostname> <port>", args[0]);
    }

    let hostname: String = args[1].parse().expect("Invalid hostname");
    let port: u16 = args[2].parse().expect("Invalid port number");

    // P2P server
    // + 12 because "peer to peer" is 12 characters long
    let datastore = Arc::new(DataStore::new());
    P2PServer::new(datastore, &hostname, port + 12)
        .expect("Failed to create and start P2P server");

    ProtocolClient::new(&hostname, port)
}

fn basic_usage() -> Result<(), ProtocolClientError> {
    let mut protocol_client = create_protocol_client()?;

    let mut _data = "my_data".to_string();
    let data_key = 1;

    protocol_client.registry_create(data_key)?;
    let _ = protocol_client.registry_create(data_key);
    protocol_client.registry_write_sync(data_key)?;

    info!("Write of the data with key {}", data_key);
    _data = "modified_data".to_string();

    protocol_client.registry_release(data_key)?;

    let holder = protocol_client.registry_read_sync(data_key)?;

    // tODO: missing peer to peer implementation to get the data content
    info!("Read of the data with key {}, holder: {}", data_key, holder);

    protocol_client.registry_release(data_key)?;
    protocol_client.registry_delete(data_key)?;

    Ok(())
}

fn advanced_usage() -> Result<(), ProtocolClientError> {
    let mut protocol_client = create_protocol_client()?;
    let data_key = 2;

    let _ = protocol_client.registry_create(data_key);

    protocol_client.registry_write_sync(data_key)?;

    protocol_client.registry_read(data_key)?;
    handle_wait_error!(protocol_client.registry_await_read(), {
        warn!("Wait error for read access of {}", data_key);
        protocol_client.registry_release(data_key)?;
        protocol_client.registry_await_read()?;
    });
    protocol_client.registry_release(data_key)?;

    let holder = protocol_client.registry_read_sync(data_key)?;
    info!("Holder of the key: {:?}", holder);

    protocol_client.registry_release(data_key)?;
    protocol_client.registry_delete(data_key)?;

    Ok(())
}

fn main() -> Result<(), ProtocolClientError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();

    let handles =
        vec![thread::spawn(basic_usage), thread::spawn(advanced_usage)];

    for handle in handles {
        let _ = handle
            .join()
            .expect("Failed to join thread")
            .inspect_err(|err| error!("Thread error: {}", err));
    }

    Ok(())
}
