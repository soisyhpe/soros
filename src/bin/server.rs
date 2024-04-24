use std::env;

use env_logger::Env;
use soros::registry_server::{RegistryServer, RegistryServerError};

fn main() -> Result<(), RegistryServerError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <port>", args[0]);
        return Ok(());
    }

    let port: u16 = args[1].parse().expect("Invalid port number");
    let mut registry_server = RegistryServer::new(port)?;
    registry_server.bind()?;

    Ok(())
}
