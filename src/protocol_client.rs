use log::{error, info, warn};
use std::{io::prelude::*, net::TcpStream};
use thiserror::Error;

use crate::{
    protocol::{
        KeyId, Message, ProcId, RegistryMessage, RegistryResponse, RequestType,
    },
    registry_request, registry_stop,
};

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ProtocolClientError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("registry error -> {0}")]
    RegistryError(String),
    #[error("unexpected response")]
    UnexpectedResponse,
    #[error("wait error: {0}")]
    WaitError(KeyId),
}

#[derive(Debug)]
pub struct ProtocolClient {
    pub hostname: String,
    pub port: u32,
    pub proc_id: ProcId,
    registry_stream: TcpStream,
    curr_data: Vec<u8>,
}

impl ProtocolClient {
    pub fn new(hostname: &str, port: u32) -> Result<Self, ProtocolClientError> {
        info!("Trying to connect to the registry: {}:{}", hostname, port);
        let mut registry_stream =
            TcpStream::connect(format!("{}:{}", hostname, port))?;
        let mut curr_data = Vec::new();

        let proc_id = ProtocolClient::registry_handle_connection(
            &mut registry_stream,
            &mut curr_data,
        )?;

        Ok(Self {
            curr_data,
            proc_id,
            hostname: hostname.to_string(),
            port,
            registry_stream,
        })
    }

    fn receive_message(
        stream: &mut TcpStream,
        curr_data: &mut Vec<u8>,
    ) -> Result<Message, ProtocolClientError> {
        loop {
            // While testing in local, the stream reading sometimes block.
            // Lowering the buffer size seems to  fix the issue...
            let mut buffer = [0; 32];
            let len = stream.read(&mut buffer)?;
            curr_data.extend_from_slice(&buffer[..len]);
            if buffer.contains(&b'\n') {
                break;
            }
        }

        let index = curr_data
            .iter()
            .position(|&c| c == b'\n')
            .ok_or_else(|| ProtocolClientError::UnexpectedResponse)?;
        let data = &curr_data[..index].to_vec();
        curr_data.drain(..index + 1);

        let message =
            Message::from_slice(data.as_slice()).inspect_err(|_| {
                error!(
                    "data: {:?}",
                    std::str::from_utf8(data.as_slice()).unwrap()
                );
            })?;
        Ok(message)
    }

    /// Wait until read request is granted.
    pub fn registry_await_read(
        &mut self,
        key_id: KeyId,
    ) -> Result<ProcId, ProtocolClientError> {
        self.registry_expect_holder(key_id)
    }

    /// Wait until write request is granted.
    pub fn registry_await_write(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        self.registry_expect_success(key_id)
    }

    /// Send a create request to the registry.
    pub fn registry_create(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry create: {}", self.proc_id, key_id);
        self.registry_send_request(key_id, RequestType::Create)?;
        self.registry_expect_success(key_id)
    }

    /// Send a delete request to the registry.
    pub fn registry_delete(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry delete: {}", self.proc_id, key_id);
        self.registry_send_request(key_id, RequestType::Delete)?;
        self.registry_expect_success(key_id)
    }

    /// Expect a response from the registry with the holder of the specified key.
    fn registry_expect_holder(
        &mut self,
        key_id: KeyId,
    ) -> Result<ProcId, ProtocolClientError> {
        match self.registry_handle_response()? {
            RegistryResponse::Holder(resp_key_id, proc_id)
                if key_id == resp_key_id =>
            {
                Ok(proc_id)
            }
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    /// Expect a success response from the registry for the specified key.
    fn registry_expect_success(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        match self.registry_handle_response()? {
            RegistryResponse::Success(resp_key_id) if key_id == resp_key_id => {
                Ok(())
            }
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    /// Handle the initial connection to the registry, retrieving the process id.
    fn registry_handle_connection(
        registry_stream: &mut TcpStream,
        curr_data: &mut Vec<u8>,
    ) -> Result<ProcId, ProtocolClientError> {
        match ProtocolClient::receive_message(registry_stream, curr_data)? {
            Message::Registry(RegistryMessage::Connection(proc_id)) => {
                Ok(proc_id)
            }
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    fn registry_handle_response(
        &mut self,
    ) -> Result<RegistryResponse, ProtocolClientError> {
        match ProtocolClient::receive_message(
            &mut self.registry_stream,
            &mut self.curr_data,
        )? {
            Message::Registry(RegistryMessage::Response(resp)) => match resp {
                RegistryResponse::Error(err) => {
                    Err(ProtocolClientError::RegistryError(err))
                }
                RegistryResponse::Wait(key_id) => {
                    Err(ProtocolClientError::WaitError(key_id))
                }
                _ => Ok(resp),
            },
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    /// Send a read request to the registry.
    pub fn registry_read(
        &mut self,
        key_id: KeyId,
    ) -> Result<ProcId, ProtocolClientError> {
        info!("{} -> Registry read: {:?}", self.proc_id, key_id);
        self.registry_send_request(key_id, RequestType::Read)?;
        self.registry_expect_holder(key_id)
    }

    /// Send a read request to the registry, wait until it's granted.
    pub fn registry_read_sync(
        &mut self,
        key_id: KeyId,
    ) -> Result<ProcId, ProtocolClientError> {
        info!("{} -> Registry read sync: {}", self.proc_id, key_id);
        match self.registry_read(key_id) {
            Err(ProtocolClientError::WaitError(key_id)) => {
                warn!("Waiting for read of {}...", key_id);
                self.registry_expect_holder(key_id)
            }
            res => res,
        }
    }

    /// Send a release request to the registry.
    pub fn registry_release(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry release: {}", self.proc_id, key_id);
        self.registry_send_request(key_id, RequestType::Release)?;
        self.registry_expect_success(key_id)
    }

    fn registry_send_request(
        &mut self,
        key_id: KeyId,
        request_type: RequestType,
    ) -> Result<(), ProtocolClientError> {
        let message = registry_request!(self.proc_id, key_id, request_type);
        let data = message.to_vec().map_err(ProtocolClientError::from)?;
        self.registry_stream.write_all(&data)?;
        Ok(())
    }

    /// Send a stop request to the registry, for testing purpose.
    pub fn registry_stop(&mut self) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry stop", self.proc_id);
        let message = registry_stop!();
        let data = message.to_vec().map_err(ProtocolClientError::from)?;
        self.registry_stream.write_all(&data)?;
        Ok(())
    }

    /// Send a write request to the registry.
    pub fn registry_write(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry write: {}", self.proc_id, key_id);
        self.registry_send_request(key_id, RequestType::Write)?;
        self.registry_expect_success(key_id)
    }

    /// Send a write request to the registry, wait until it's granted.
    pub fn registry_write_sync(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry write sync: {}", self.proc_id, key_id);
        match self.registry_write(key_id) {
            Err(ProtocolClientError::WaitError(key_id)) => {
                warn!("Waiting for write...");
                self.registry_expect_success(key_id)
            }
            res => res,
        }
    }
}

#[macro_export]
macro_rules! handle_wait {
    ($res: expr, $logic: expr) => {
        if let Err(ProtocolClientError::WaitError(data_key)) = $res {
            $logic
        }
    };
}
