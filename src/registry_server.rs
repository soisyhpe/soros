use crate::{
    access_manager::AccessManager,
    protocol::{Message, RegistryMessage, RegistryResponse},
};
use log::{error, info};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum RegistryServerError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Unexpected request")]
    UnexpectedRequest,
}

#[derive(Debug)]
pub struct RegistryServer {
    pub hostname: String,
    pub port: u32,
    _access_manager: AccessManager,
}

impl RegistryServer {
    pub fn new(hostname: &str, port: u32) -> Self {
        Self {
            hostname: hostname.to_string(),
            port,
            _access_manager: AccessManager::new(Box::new(|_, _, _| {})),
        }
    }

    pub fn bind(&mut self) -> Result<(), RegistryServerError> {
        info!(
            "Starting registry server on {}:{}",
            self.hostname, self.port
        );
        let listener =
            TcpListener::bind(format!("{}:{}", self.hostname, self.port))?;
        for stream in listener.incoming() {
            let _ = self
                .handle_connection(&mut stream?)
                .map_err(|err| error!("{}", err.to_string()));
        }
        Ok(())
    }

    fn handle_connection(
        &mut self,
        stream: &mut TcpStream,
    ) -> Result<(), RegistryServerError> {
        info!("handling request for {:?}", stream);
        let mut buffer = [0; 256];
        let size = stream.read(&mut buffer)?;
        let message: Message = serde_json::from_slice(&buffer[..size])?;
        let response = self.handle_request(message)?;
        stream.write_all(&serde_json::to_vec(&response)?)?;
        Ok(())
    }

    fn handle_request(
        &mut self,
        request: Message,
    ) -> Result<Message, RegistryServerError> {
        info!("{:?}", request);
        Ok(Message::Registry(RegistryMessage::Response(
            RegistryResponse::Error("Wut".to_string()),
        )))
    }
}
