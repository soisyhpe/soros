use std::env;
use std::net::SocketAddr;

use env_logger::Env;
use soros::registry_server::{RegistryServer, RegistryServerError};

fn main() -> Result<(), RegistryServerError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <primary_port> [secondary_host:secondary_:port]", args[0]);
        return Ok(());
    }

    let primary_port: u16 = args[1].parse().expect("Invalid primary port number");
    let secondary_server: Option<SocketAddr> = args.get(2).and_then(|s| s.parse().ok());

    let mut registry_server = RegistryServer::new(primary_port, secondary_server)?;
    registry_server.bind()?;

    Ok(())
}
