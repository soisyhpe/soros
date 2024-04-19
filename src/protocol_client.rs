use log::info;
use std::io::prelude::*;
use std::net::TcpStream;
use thiserror::Error;

use crate::{
    protocol::{
        KeyId, Message, ProcId, RegistryMessage, RegistryResponse, RequestType,
    },
    registry_request,
};

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ProtocolClientError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Registry error: {0}")]
    RegistryError(String),
    #[error("Unexpected response")]
    UnexpectedResponse,
}

#[derive(Debug)]
pub struct ProtocolClient {
    pub hostname: String,
    pub port: u32,
    pub proc_id: ProcId,
}

impl ProtocolClient {
    pub fn new(
        proc_id: ProcId,
        hostname: &str,
        port: u32,
    ) -> Result<Self, ProtocolClientError> {
        info!("Connecting to registry on {}:{}", hostname, port);
        Ok(Self {
            proc_id,
            hostname: hostname.to_string(),
            port,
        })
    }

    fn create_stream(&self) -> Result<TcpStream, ProtocolClientError> {
        let stream =
            TcpStream::connect(format!("{}:{}", self.hostname, self.port))?;
        Ok(stream)
    }

    pub fn registry_create(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("Registry create: {:?}", key_id);
        let mut stream = self.create_stream()?;
        self.registry_create_request(&mut stream, key_id, RequestType::Create)?;
        self.registry_expect_success(&mut stream)
    }

    fn registry_create_request(
        &mut self,
        stream: &mut TcpStream,
        key_id: KeyId,
        request_type: RequestType,
    ) -> Result<(), ProtocolClientError> {
        let message = registry_request!(self.proc_id, key_id, request_type);
        let data =
            serde_json::to_vec(&message).map_err(ProtocolClientError::from)?;
        stream.write_all(&data)?;
        Ok(())
    }

    pub fn registry_delete(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("Registry delete: {:?}", key_id);
        let mut stream = self.create_stream()?;
        self.registry_create_request(&mut stream, key_id, RequestType::Delete)?;
        self.registry_expect_success(&mut stream)
    }

    fn registry_expect_holder(
        &self,
        stream: &mut TcpStream,
    ) -> Result<ProcId, ProtocolClientError> {
        match self.registry_handle_response(stream)? {
            RegistryResponse::Holder(proc_id) => Ok(proc_id),
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    fn registry_expect_success(
        &self,
        stream: &mut TcpStream,
    ) -> Result<(), ProtocolClientError> {
        match self.registry_handle_response(stream)? {
            RegistryResponse::Success => Ok(()),
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    fn registry_handle_response(
        &self,
        stream: &mut TcpStream,
    ) -> Result<RegistryResponse, ProtocolClientError> {
        let mut buffer = [0; 256];
        let size = stream.read(&mut buffer)?;
        let message = serde_json::from_slice(&buffer[..size])?;
        match message {
            Message::Registry(RegistryMessage::Response(resp)) => match resp {
                RegistryResponse::Error(err) => {
                    Err(ProtocolClientError::RegistryError(err))
                }
                _ => Ok(resp),
            },
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    pub fn registry_request_read(
        &mut self,
        key_id: KeyId,
    ) -> Result<ProcId, ProtocolClientError> {
        info!("Registry read: {:?}", key_id);
        let mut stream = self.create_stream()?;
        self.registry_create_request(&mut stream, key_id, RequestType::Read)?;
        self.registry_expect_holder(&mut stream)
    }

    pub fn registry_request_write(
        &mut self,
        key_id: KeyId,
    ) -> Result<ProcId, ProtocolClientError> {
        info!("Registry write: {:?}", key_id);
        let mut stream = self.create_stream()?;
        self.registry_create_request(&mut stream, key_id, RequestType::Write)?;
        self.registry_expect_holder(&mut stream)
    }

    pub fn registry_request_release(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("Registry release: {:?}", key_id);
        let mut stream = self.create_stream()?;
        self.registry_create_request(
            &mut stream,
            key_id,
            RequestType::Release,
        )?;
        self.registry_expect_success(&mut stream)
    }
}
