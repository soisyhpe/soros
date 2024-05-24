use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use std::{env, thread};

use env_logger::Env;
use log::{error, info, warn};
use soros::p2p_server::{DataStore, DataStoreError, P2PServer, P2PServerError};
use soros::protocol::KeyId;
use soros::{
    handle_wait_error,
    protocol_client::{ProtocolClient, ProtocolClientError},
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("ProtocolClient error: {0}")]
    ProtocolClientError(#[from] ProtocolClientError),

    #[error("P2PServer error: {0}")]
    P2PServerError(#[from] P2PServerError),

    #[error("DataStore error: {0}")]
    DataStoreError(#[from] DataStoreError),
}

fn create_protocol_client() -> Result<ProtocolClient, ProtocolClientError> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Usage: {} <primary hostname:primary port> <secondary hostname:secondary port>", args[0]);
    }

    let primary_server: SocketAddr =
        args[1].parse().expect("Invalid primary server information");
    let secondary_server: SocketAddr = args[2]
        .parse()
        .expect("Invalid secondary server information");

    ProtocolClient::new(primary_server, secondary_server)
}

// P2P tests

fn p2p_client1() -> Result<(), ClientError> {
    let mut client = create_protocol_client()?;
    client.logging = false;
    let shared_datastore = Arc::new(DataStore::new());
    let datastore = shared_datastore.clone();
    let data_key: KeyId = 1;
    info!("client 1 -> proc id {}", client.proc_id);

    let _ = thread::spawn(|| -> Result<(), P2PServerError> {
        let mut server = P2PServer::new(
            shared_datastore,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 6001),
        )?;
        server.bind()?;
        Ok(())
    });

    // create request
    client.registry_create(data_key)?;
    datastore.create(data_key, "inital data fom client 1")?;
    info!(
        "client 1 -> data with key {} created with content \"{:?}\"",
        data_key,
        datastore.get(&data_key)
    );

    thread::sleep(Duration::from_secs(2));
    let _ = client.registry_create(data_key);

    // write request
    info!("client 1 -> try to write {}", data_key);
    client.registry_write_sync(data_key)?;
    datastore.write(&data_key, "new data from client 1")?;
    info!(
        "client 1 -> data with key {} written with content \"{:?}\"",
        data_key,
        datastore.get(&data_key)
    );

    thread::sleep(Duration::from_secs(5));

    // release request
    client.registry_release(data_key)?;
    info!("client 1 -> release write access with key {}", data_key);

    thread::sleep(Duration::from_secs(6));

    // stop client
    info!("client 1 -> stopped");

    Ok(())
}

fn p2p_client2() -> Result<(), ClientError> {
    thread::sleep(Duration::from_secs(1));
    let mut client = create_protocol_client()?;
    client.logging = false;
    let shared_datastore = Arc::new(DataStore::new());
    let datastore = shared_datastore.clone();
    let data_key: KeyId = 1;
    info!("client 2 -> proc id {}", client.proc_id);

    let _ = thread::spawn(|| -> Result<(), P2PServerError> {
        let mut server = P2PServer::new(
            shared_datastore,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 6002),
        )?;
        server.bind()?;
        Ok(())
    });

    thread::sleep(Duration::from_secs(2));

    // try to create
    let _ = client.registry_create(data_key);

    // read request
    let _owner_addr = client.registry_read_sync(data_key)?;
    // In real condition, client are on different computers and should run the
    // p2p server on a defined port
    let owner_addr =
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 6001);

    info!(
        "client2 -> read for key {} granted, owned by {:?}",
        data_key, owner_addr
    );

    let data = client.p2p_read(data_key, owner_addr)?;
    datastore.create(data_key, &data)?;
    info!(
        "client 1 -> retrieved from {:?}, data with key {}, read with content \"{:?}\"",
        owner_addr,
        data_key,
        datastore.get(&data_key)
    );

    // release
    client.registry_release(data_key)?;
    info!("client 2 -> release read access with key {}", data_key);

    // delete
    client.registry_delete(data_key)?;
    info!("client 2 -> delete key {}", data_key);

    // stop client
    info!("client 2 -> stopped");

    Ok(())
}

fn p2p() -> Result<(), ClientError> {
    let handle_client1 = thread::spawn(p2p_client1);
    let handle_client2 = thread::spawn(p2p_client2);
    if let Err(err) = handle_client1.join().unwrap() {
        error!("client 1 -> {}", err);
    }
    if let Err(err) = handle_client2.join().unwrap() {
        error!("client 2 -> {}", err);
    }
    Ok(())
}

// Simple tests

fn _basic_usage() -> Result<(), ClientError> {
    let mut protocol_client = create_protocol_client()?;

    let data_key = 1;
    let mut _data = "my_data".to_string();

    protocol_client.registry_create(data_key)?;
    let _ = protocol_client.registry_create(data_key);
    protocol_client.registry_write_sync(data_key)?;

    info!("Write of the data with key {}", data_key);
    _data = "modified_data".to_string();

    protocol_client.registry_release(data_key)?;

    let holder = protocol_client.registry_read_sync(data_key)?;

    // TODO: missing peer to peer implementation to get the data content
    info!("Read of the data with key {}, holder: {}", data_key, holder);

    protocol_client.registry_release(data_key)?;
    protocol_client.registry_delete(data_key)?;

    // Now primary server should be down for demonstration purpose

    info!("Now primary server should be down !");

    // Try to create new data
    let data_key: KeyId = 2;
    let mut _data = "second_data".to_string();
    protocol_client.registry_create(2)?;
    protocol_client.registry_write_sync(data_key)?;

    info!("Write of the data with key {}", data_key);

    protocol_client.registry_release(data_key)?;

    Ok(())
}

fn _advanced_usage() -> Result<(), ClientError> {
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

fn main() -> Result<(), ClientError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();

    let handles = vec![
        thread::spawn(p2p),
        // thread::spawn(basic_usage),
        // thread::spawn(advanced_usage)
    ];

    for handle in handles {
        let _ = handle
            .join()
            .expect("Failed to join thread")
            .inspect_err(|err| error!("Thread error: {}", err));
    }

    Ok(())
}
