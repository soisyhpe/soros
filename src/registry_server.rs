use crate::{
    access_manager::{AccessManager, AccessManagerError},
    protocol::{Message, RegistryMessage, RegistryResponse, RequestType},
    registry_response,
};
use log::{error, info};
use mio::{
    net::{TcpListener, TcpStream},
    Events, Interest, Poll, Token,
};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::ToSocketAddrs,
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
        let addr_string = format!("{}:{}", self.hostname, self.port);
        let addr = addr_string.to_socket_addrs()?.next().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                format!("Could not find address {}", addr_string),
            )
        })?;
        let mut listener = TcpListener::bind(addr)?;

        // mio poll, use epoll / kqueue under the hood
        let mut events = Events::with_capacity(128);
        let mut poll = Poll::new()?;
        const LISTENER: Token = Token(0);
        poll.registry().register(
            &mut listener,
            LISTENER,
            Interest::READABLE,
        )?;

        // keep track of clients
        let mut counter: usize = 0;
        let mut clients: HashMap<Token, TcpStream> = HashMap::new();

        loop {
            poll.poll(&mut events, None)?;
            for event in events.iter() {
                match event.token() {
                    LISTENER => {
                        let (mut stream, _addr) = listener.accept()?;

                        counter += 1;
                        let token = Token(counter);
                        poll.registry().register(
                            &mut stream,
                            token,
                            Interest::READABLE,
                        )?;

                        clients.insert(token, stream);
                    }
                    token if event.is_readable() => {
                        let stream = clients.get_mut(&token).unwrap();
                        let mut buffer = [0; 256];
                        let read = stream.read(&mut buffer);
                        match read {
                            Ok(0) => {
                                clients.remove(&token);
                                break;
                            }
                            Ok(size) => {
                                if let Err(err) =
                                    self.handle_request(stream, &buffer[..size])
                                {
                                    self.handle_error(stream, err)?;
                                }
                            }
                            Err(ref e)
                                if e.kind() == io::ErrorKind::WouldBlock =>
                            {
                                break
                            }
                            Err(e) => panic!("Unexpected error: {}", e),
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_error(
        &mut self,
        stream: &mut TcpStream,
        err: RegistryServerError,
    ) -> Result<(), RegistryServerError> {
        error!("{}", err.to_string());
        self.handle_response(stream, RegistryResponse::Error(err.to_string()))
    }

    fn _handle_connection(
        &mut self,
        stream: &mut TcpStream,
    ) -> Result<(), RegistryServerError> {
        info!("handling connection from {:?}", stream.peer_addr());
        Ok(())
    }

    fn handle_request(
        &mut self,
        stream: &mut TcpStream,
        data: &[u8],
    ) -> Result<(), RegistryServerError> {
        let message: Message = serde_json::from_slice(data)?;
        let response = self.handle_message(message)?;
        self.handle_response(stream, response)
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
            }) => {
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
                        .map(RegistryResponse::Holder),
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
            _ => Err(RegistryServerError::UnexpectedRequest),
        }
    }

    fn handle_response(
        &self,
        stream: &mut TcpStream,
        response: RegistryResponse,
    ) -> Result<(), RegistryServerError> {
        let data = serde_json::to_vec(&registry_response!(response))?;
        stream.write_all(&data)?;
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
