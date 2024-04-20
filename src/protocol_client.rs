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
    #[error("Wait error")]
    WaitError,
}

#[derive(Debug)]
pub struct ProtocolClient {
    pub hostname: String,
    pub port: u32,
    pub proc_id: ProcId,
    stream: TcpStream,
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
            stream: TcpStream::connect(format!("{}:{}", hostname, port))?,
        })
    }

    pub fn registry_create(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("Registry create: {:?}", key_id);
        self.registry_send_request(key_id, RequestType::Create)?;
        self.registry_expect_success()
    }

    fn registry_send_request(
        &mut self,
        key_id: KeyId,
        request_type: RequestType,
    ) -> Result<(), ProtocolClientError> {
        let message = registry_request!(self.proc_id, key_id, request_type);
        let data =
            serde_json::to_vec(&message).map_err(ProtocolClientError::from)?;
        self.stream.write_all(&data)?;
        Ok(())
    }

    pub fn registry_delete(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("Registry delete: {:?}", key_id);
        self.registry_send_request(key_id, RequestType::Delete)?;
        self.registry_expect_success()
    }

    pub fn registry_expect_holder(
        &mut self,
    ) -> Result<ProcId, ProtocolClientError> {
        match self.registry_handle_response()? {
            RegistryResponse::Holder(proc_id) => Ok(proc_id),
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    pub fn registry_expect_success(
        &mut self,
    ) -> Result<(), ProtocolClientError> {
        match self.registry_handle_response()? {
            RegistryResponse::Success => Ok(()),
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    fn registry_handle_response(
        &mut self,
    ) -> Result<RegistryResponse, ProtocolClientError> {
        let mut buffer = [0; 256];
        let size = self.stream.read(&mut buffer)?;
        let message = serde_json::from_slice(&buffer[..size])?;
        match message {
            Message::Registry(RegistryMessage::Response(resp)) => match resp {
                RegistryResponse::Error(err) => {
                    Err(ProtocolClientError::RegistryError(err))
                }
                RegistryResponse::Wait => Err(ProtocolClientError::WaitError),
                _ => Ok(resp),
            },
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    pub fn registry_read(
        &mut self,
        key_id: KeyId,
    ) -> Result<ProcId, ProtocolClientError> {
        info!("Registry read: {:?}", key_id);
        self.registry_send_request(key_id, RequestType::Read)?;
        self.registry_expect_holder()
    }

    pub fn registry_release(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("Registry release: {:?}", key_id);
        self.registry_send_request(key_id, RequestType::Release)?;
        self.registry_expect_success()
    }

    pub fn registry_write(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("Registry write: {:?}", key_id);
        self.registry_send_request(key_id, RequestType::Write)?;
        self.registry_expect_success()
    }
}
