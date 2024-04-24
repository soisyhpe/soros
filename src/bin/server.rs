use env_logger::Env;
use soros::registry_server::{RegistryServer, RegistryServerError};

fn main() -> Result<(), RegistryServerError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .init();
    let mut registry_server = RegistryServer::new("localhost", 8888)?;
    registry_server.bind()?;
    Ok(())
}
