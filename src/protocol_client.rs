use log::{debug, error, info, warn};
use std::time::Duration;
use std::{
    io::prelude::*,
    net::{SocketAddr, TcpStream},
};
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

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Registry error: {0}")]
    RegistryError(String),

    #[error("Unexpected response")]
    UnexpectedResponse,

    #[error("Wait error for key id: {0}")]
    WaitError(KeyId),

    #[error("No backup server available")]
    NoBackupServer(),
}

#[derive(Debug)]
pub struct ProtocolClient {
    is_primary: bool,

    pub primary_server: SocketAddr,
    pub secondary_server: SocketAddr,

    pub proc_id: ProcId,
    registry_stream: TcpStream,
    curr_data: Vec<u8>,
}

impl ProtocolClient {
    pub fn new(
        primary_server: SocketAddr,
        secondary_server: SocketAddr,
    ) -> Result<Self, ProtocolClientError> {
        info!(
            "Trying to connect to the primary registry: {:?}",
            primary_server
        );

        let mut is_primary = true;
        let mut registry_stream = match TcpStream::connect(primary_server) {
            // If the connection is successful, return the stream
            Ok(stream) => stream,

            // If the primary server is down, switch to the secondary server
            Err(e)
                if e.kind() == std::io::ErrorKind::ConnectionRefused
                    || e.kind() == std::io::ErrorKind::ConnectionReset
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                warn!(
                    "Failed to connect to primary server: {:?}",
                    primary_server
                );
                info!("Switching to secondary server: {:?}", secondary_server);

                is_primary = false;
                TcpStream::connect(secondary_server)?
            }

            // If the error is not a connection refused, return the error
            Err(e) => return Err(ProtocolClientError::IoError(e)),
        };

        // 5 seconds timeout for the registry
        let timeout = Duration::from_secs(5);
        registry_stream.set_read_timeout(Some(timeout))?;
        registry_stream.set_write_timeout(Some(timeout))?;

        let mut curr_data = Vec::new();

        let proc_id = ProtocolClient::registry_handle_connection(
            &mut registry_stream,
            &mut curr_data,
        )?;

        Ok(Self {
            is_primary,

            primary_server,
            secondary_server,

            registry_stream,
            proc_id,
            curr_data,
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

        debug!(
            "Parsed data: {:?}",
            std::str::from_utf8(data.as_slice()).unwrap()
        );

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
    ) -> Result<(KeyId, SocketAddr), ProtocolClientError> {
        match self.registry_handle_response()? {
            RegistryResponse::Holder(key_id, addr) => Ok((key_id, addr)),
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    /// Wait until write request is granted.
    pub fn registry_await_write(
        &mut self,
    ) -> Result<KeyId, ProtocolClientError> {
        match self.registry_handle_response()? {
            RegistryResponse::Success(key_id) => Ok(key_id),
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
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
        ) {
            Ok(Message::Registry(RegistryMessage::Response(resp))) => {
                match resp {
                    RegistryResponse::Error(err) => {
                        Err(ProtocolClientError::RegistryError(err))
                    }
                    RegistryResponse::Wait(key_id) => {
                        Err(ProtocolClientError::WaitError(key_id))
                    }
                    _ => Ok(resp),
                }
            }
            Err(ProtocolClientError::IoError(err)) => match err.kind() {
                std::io::ErrorKind::ConnectionReset => {
                    self.switch_to_secondary()?;
                    self.registry_handle_response()
                }
                _ => {
                    info!("error : {:?}", err.kind());
                    Err(ProtocolClientError::UnexpectedResponse)
                }
            },
            _ => Err(ProtocolClientError::UnexpectedResponse),
        }
    }

    /// Send a read request to the registry.
    pub fn registry_read(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry read: {:?}", self.proc_id, key_id);
        self.registry_send_request(key_id, RequestType::Read)
    }

    /// Send a read request to the registry, wait until it's granted.
    pub fn registry_read_sync(
        &mut self,
        key_id: KeyId,
    ) -> Result<SocketAddr, ProtocolClientError> {
        self.registry_read(key_id)?;
        match self.registry_await_read() {
            Err(ProtocolClientError::WaitError(key_id)) => {
                warn!("Awaiting read of {}...", key_id);
                let res = self.registry_await_read()?;
                if key_id != res.0 {
                    return Err(ProtocolClientError::UnexpectedResponse);
                }
                Ok(res)
            }
            res => res,
        }
        .map(|(_, addr)| addr)
    }

    /// Send a release request to the registry.
    pub fn registry_release(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry release: {}", self.proc_id, key_id);
        self.registry_send_request(key_id, RequestType::Release)?; // <----
        self.registry_expect_success(key_id) // ?????
    }

    fn switch_to_secondary(&mut self) -> Result<(), ProtocolClientError> {
        warn!("Primary server is not responding");
        info!("Switching to secondary server: {:?}", self.secondary_server);

        self.registry_stream = TcpStream::connect(self.secondary_server)?;
        let ok = ProtocolClient::registry_handle_connection(
            &mut self.registry_stream,
            &mut self.curr_data,
        );

        // 5 seconds timeout for the registry
        let timeout = Duration::from_secs(1);
        self.registry_stream.set_read_timeout(Some(timeout))?;
        self.registry_stream.set_write_timeout(Some(timeout))?;

        Ok(())
    }

    fn registry_send_request(
        &mut self,
        key_id: KeyId,
        request_type: RequestType,
    ) -> Result<(), ProtocolClientError> {
        let message = registry_request!(key_id, self.proc_id, request_type);
        let data = message.to_vec().map_err(ProtocolClientError::from)?;

        // Try to send the message to the registry
        self.registry_stream.write_all(&data)?;

        // Check if connection is still active
        let mut buf = [0; 1];
        match self.registry_stream.peek(&mut buf) {
            Ok(_) => Ok(()),
            Err(e) => {
                // If there is no backup server, return an error
                if !self.is_primary {
                    return Err(ProtocolClientError::NoBackupServer());
                }

                // If the primary server is down, switch to the secondary server
                if e.kind() == std::io::ErrorKind::ConnectionReset
                    || e.kind() == std::io::ErrorKind::ConnectionRefused
                    || e.kind() == std::io::ErrorKind::TimedOut
                {
                    self.switch_to_secondary()?;

                    // Retry to send the message to the registry
                    info!("Forwarding previous request to secondary server");
                    self.registry_stream.write_all(&data)?;

                    Ok(())
                } else {
                    // If the error is not a connection refused, return the error
                    Err(ProtocolClientError::from(e))
                }
            }
        }
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
        self.registry_send_request(key_id, RequestType::Write)
    }

    /// Send a write request to the registry, wait until it's granted.
    pub fn registry_write_sync(
        &mut self,
        key_id: KeyId,
    ) -> Result<(), ProtocolClientError> {
        info!("{} -> Registry write sync: {}", self.proc_id, key_id);
        self.registry_write(key_id)?;
        match self.registry_await_write() {
            Err(ProtocolClientError::WaitError(key_id)) => {
                warn!("Awaiting write of {}...", key_id);
                let res_key_id = self.registry_await_write()?;
                if key_id != res_key_id {
                    return Err(ProtocolClientError::UnexpectedResponse);
                }
                Ok(res_key_id)
            }
            res => res,
        }
        .map(|_| ())
    }
}

#[macro_export]
macro_rules! handle_wait_error {
    ($res: expr, $logic: block) => {
        if let Err(ProtocolClientError::WaitError(_)) = $res {
            $logic
        }
    };
}
