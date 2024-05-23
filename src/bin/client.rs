use std::net::{IpAddr, SocketAddr};
use std::{env, thread};

use env_logger::Env;
use log::{error, info, warn};
use soros::p2p_server::{DataStoreError, P2PServer, P2PServerError};
use soros::protocol::KeyId;
use soros::{
    handle_wait_error,
    protocol_client::{ProtocolClient, ProtocolClientError},
    registry_stop,
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

// fn p2p() -> Result<(), ClientError> {
//     // client1
//     let mut client1 = create_protocol_client()?;
//     let client1_datastore = Arc::new(DataStore::new());
//     let data_store1 = client1_datastore.clone();
//
//     let handle1 = thread::spawn(|| -> Result<(), P2PServerError> {
//         let mut client1_p2p = P2PServer::new(data_store1, "127.0.0.1", 6001)?;
//         client1_p2p.bind()?;
//         Ok(())
//     });
//
//     sleep(Duration::from_secs(1));
//
//     let data_key = 1;
//     client1_datastore.clone().create(data_key, "my_data")?;
//     info!(
//         "Data with key {} created with content {}",
//         data_key,
//         client1_datastore.get(&data_key)?
//     );
//
//     client1.registry_create(data_key)?;
//     let _ = client1.registry_create(data_key);
//     client1.registry_write_sync(data_key)?;
//     info!("Write of the data with key {}", data_key);
//
//     client1_datastore
//         .clone()
//         .write(&data_key, "modified_data")?;
//     info!(
//         "Data with key {} modified with content {}",
//         data_key,
//         client1_datastore.get(&data_key)?
//     );
//
//     client1.registry_release(data_key)?;
//
//     // client2
//     let mut _client2 = create_protocol_client()?;
//     // let client2_datastore = Arc::new(DataStore::new());
//     // let data_store2 = client2_datastore.clone();
//
//     // let handle2 = thread::spawn(|| -> Result<(), P2PServerError> {
//     //     let mut client2_p2p = P2PServer::new(data_store2, "localhost", 6001)?;
//     //     client2_p2p.bind()?;
//     //     Ok(())
//     // });
//
//     // TODO: let holder = client1.registry_read_sync(data_key)?;
//     let holder = SocketAddr::new(IpAddr::V4("127.0.0.1".parse().unwrap()), 6001);
//     info!("Read of the data with key {}, holder: {}", data_key, holder);
//
//     P2PServer::get(SocketAddr::V4(), data_key)?;
//     // info!("Read data {}", data);
//
//     client1.registry_delete(data_key)?;
//
//     let _ = handle1.join().unwrap();
//     // let _ = handle2.join();
//
//     Ok(())
// }

fn basic_usage() -> Result<(), ClientError> {
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

    info!("Now primary server should be down for demonstration purpose");
    // thread::sleep(std::time::Duration::from_secs(1));

    // Try to create new data
    let data_key: KeyId = 2;
    let mut _data = "second_data".to_string();
    protocol_client.registry_create(2)?;
    protocol_client.registry_write_sync(data_key)?;

    info!("Write of the data with key {}", data_key);

    protocol_client.registry_release(data_key)?;

    Ok(())
}

fn advanced_usage() -> Result<(), ClientError> {
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
        /*thread::spawn(p2p)*/
        thread::spawn(basic_usage), /*, thread::spawn(advanced_usage)*/
    ];

    for handle in handles {
        let _ = handle
            .join()
            .expect("Failed to join thread")
            .inspect_err(|err| error!("Thread error: {}", err));
    }

    Ok(())
}
