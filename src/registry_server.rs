use crate::{
    access_manager::{AccessManager, AccessManagerError},
    protocol::{
        KeyId, Message, ProcId, RegistryMessage, RegistryResponse, RequestType,
    },
    registry_response,
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
    #[error("Access manager error: {0}")]
    AccessManager(#[from] AccessManagerError),
}

#[derive(Debug)]
pub struct RegistryServer {
    pub hostname: String,
    pub port: u32,
    access_manager: AccessManager,
}

impl RegistryServer {
    pub fn bind(&mut self) -> Result<(), RegistryServerError> {
        info!(
            "Starting registry server on {}:{}",
            self.hostname, self.port
        );
        let listener =
            TcpListener::bind(format!("{}:{}", self.hostname, self.port))?;

        for stream in listener.incoming() {
            let mut stream = stream?;
            let res = self.handle_connection(&mut stream);

            if let Err(err) = res {
                error!("{}", err.to_string());
                self.handle_response(
                    &mut stream,
                    RegistryResponse::Error(err.to_string()),
                )?;
            }
        }

        Ok(())
    }

    fn handle_connection(
        &mut self,
        stream: &mut TcpStream,
    ) -> Result<(), RegistryServerError> {
        info!("handling connection: {:?}", stream);
        let mut buffer = [0; 256];
        let size = stream.read(&mut buffer)?;
        let message: Message = serde_json::from_slice(&buffer[..size])?;
        let response = self.handle_message(message)?;
        self.handle_response(stream, response)?;
        Ok(())
    }

    fn handle_message(
        &mut self,
        message: Message,
    ) -> Result<RegistryResponse, RegistryServerError> {
        info!("handling message: {:?}", message);
        match message {
            Message::Registry(RegistryMessage::Request {
                request_type,
                proc_id,
                key_id,
            }) => self.handle_request(request_type, proc_id, key_id),
            _ => Err(RegistryServerError::UnexpectedRequest),
        }
    }

    fn handle_request(
        &mut self,
        request_type: RequestType,
        proc_id: ProcId,
        key_id: KeyId,
    ) -> Result<RegistryResponse, RegistryServerError> {
        let response = match request_type {
            RequestType::Create => self
                .access_manager
                .create(proc_id, key_id)
                .map(|_| RegistryResponse::Success),
            RequestType::Delete => self
                .access_manager
                .delete(key_id)
                .map(|_| RegistryResponse::Success),
            RequestType::Read => self
                .access_manager
                .request_read(proc_id, key_id)
                .map(|_| RegistryResponse::Success),
            RequestType::Write => self
                .access_manager
                .request_write(proc_id, key_id)
                .map(|_| RegistryResponse::Success),
            RequestType::Release => self
                .access_manager
                .release(proc_id, key_id)
                .map(|_| RegistryResponse::Success),
        };
        response.map_err(RegistryServerError::AccessManager)
    }

    fn handle_response(
        &self,
        stream: &mut TcpStream,
        response: RegistryResponse,
    ) -> Result<(), RegistryServerError> {
        stream
            .write_all(&serde_json::to_vec(&registry_response!(response))?)?;
        Ok(())
    }

    pub fn new(hostname: &str, port: u32) -> Self {
        Self {
            hostname: hostname.to_string(),
            port,
            access_manager: AccessManager::new(Box::new(|_, _, _| {})),
        }
    }
}
