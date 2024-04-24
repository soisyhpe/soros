use crate::{
    access_manager::{AccessGranted, AccessManager, AccessManagerError},
    protocol::{
        Message, ProcId, RegistryMessage, RegistryResponse, RequestType,
    },
    registry_connection, registry_response,
};
use log::{debug, error, info, warn};
use mio::{
    net::{TcpListener, TcpStream},
    Events, Interest, Poll, Token,
};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr},
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum RegistryServerError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("unexpected request")]
    UnexpectedRequest,
    #[error("access manager error: {0}")]
    AccessManager(#[from] AccessManagerError),
    #[error("Invalid {:?}", .0)]
    InvalidToken(Token),
    #[error("Stop requested")]
    StopRequest,
}

#[derive(Debug)]
pub struct RegistryServer {
    addr: SocketAddr,
    access_manager: AccessManager,
    poll: Poll,
    token_stream_map: HashMap<Token, TcpStream>,
    token_addr_map: HashMap<Token, SocketAddr>,
    id_counter: ProcId,
}

impl RegistryServer {
    pub fn bind(&mut self) -> Result<(), RegistryServerError> {
        info!("Starting registry server on {:?}", self.addr);
        let mut listener = TcpListener::bind(self.addr)?;

        // mio poll, use epoll / kqueue under the hood
        let mut events = Events::with_capacity(128);
        self.poll = Poll::new()?;
        const LISTENER: Token = Token(0);
        self.poll.registry().register(
            &mut listener,
            LISTENER,
            Interest::READABLE,
        )?;

        loop {
            self.poll.poll(&mut events, None)?;
            for event in events.iter() {
                match event.token() {
                    LISTENER => loop {
                        match listener.accept() {
                            Ok((stream, addr)) => {
                                self.handle_connection(stream, addr)?
                            }
                            Err(ref e)
                                if e.kind() == io::ErrorKind::WouldBlock =>
                            {
                                break
                            }
                            Err(e) => panic!("Unexpected error: {}", e),
                        }
                    },
                    token if event.is_readable() => {
                        let res = self.handle_data(token);
                        match res {
                            Err(RegistryServerError::StopRequest) => {
                                info!("Gracefully shutdown");
                                return Ok(());
                            }
                            Err(err) => {
                                error!("Failed to handle data for token: {}, got: {}", token.0, err)
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_connection(
        &mut self,
        mut stream: TcpStream,
        addr: SocketAddr,
    ) -> Result<(), RegistryServerError> {
        self.id_counter += 1;
        let token = Token(self.id_counter);
        self.poll.registry().register(
            &mut stream,
            token,
            Interest::READABLE,
        )?;

        info!("handling connection from {:?}", stream.peer_addr());
        let data = registry_connection!(token.0).to_vec()?;
        stream.write_all(&data)?;

        self.token_stream_map.insert(token, stream);
        self.token_addr_map.insert(token, addr);

        Ok(())
    }

    fn handle_data(&mut self, token: Token) -> Result<(), RegistryServerError> {
        let stream = self.token_stream_map.get_mut(&token).unwrap();
        let mut buffer = [0; 256];
        let read = stream.read(&mut buffer);

        match read {
            Ok(0) => {
                self.remove_client(token);
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {
                self.remove_client(token);
                Err(RegistryServerError::IoError(e))
            }
            Ok(size) => {
                let res = self.handle_request(token, &buffer[..size]);
                match res {
                    Err(RegistryServerError::StopRequest) => return res,
                    Err(err) => self.handle_error(token, err)?,
                    _ => {}
                }
                Ok(())
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(RegistryServerError::IoError(e)),
        }
    }

    fn handle_error(
        &mut self,
        token: Token,
        err: RegistryServerError,
    ) -> Result<(), RegistryServerError> {
        error!("{}", err.to_string());
        self.handle_response(token, RegistryResponse::Error(err.to_string()))
    }

    fn handle_message(
        &mut self,
        message: Message,
        proc_id: ProcId,
    ) -> Result<RegistryResponse, RegistryServerError> {
        debug!("handling message: {:?}", message);
        match message {
            Message::Registry(RegistryMessage::StopRequest) => {
                Err(RegistryServerError::StopRequest)
            }
            Message::Registry(RegistryMessage::Request {
                request_type,
                key_id,
            }) => {
                let response = match request_type {
                    RequestType::Create => self
                        .access_manager
                        .create(proc_id, key_id)
                        .map(|_| RegistryResponse::Success(key_id)),
                    RequestType::Delete => self
                        .access_manager
                        .delete(key_id)
                        .map(|_| RegistryResponse::Success(key_id)),
                    RequestType::Read => {
                        match self.access_manager.read(proc_id, key_id) {
                            Ok(holder_id) => {
                                let addr = self.proc_id_to_addr(holder_id)?;
                                Ok(RegistryResponse::Holder(key_id, addr))
                            }
                            Err(AccessManagerError::RequestAccess(
                                proc_id,
                                key_id,
                            )) => {
                                warn!("Read access currently impossible for proc {}, key {}", proc_id, key_id);
                                Ok(RegistryResponse::Wait(key_id))
                            }
                            Err(err) => Err(err),
                        }
                    }
                    RequestType::Write => {
                        let err = self
                            .access_manager
                            .write(proc_id, key_id)
                            .map(|_| RegistryResponse::Success(key_id));
                        match err {
                            Err(AccessManagerError::RequestAccess(_, _)) => {
                                warn!("Write access currently impossible for proc {}, key {}", proc_id, key_id);
                                Ok(RegistryResponse::Wait(key_id))
                            }
                            _ => err,
                        }
                    }
                    RequestType::Release => self
                        .access_manager
                        .release(proc_id, key_id)
                        .map(|_| RegistryResponse::Success(key_id)),
                };
                response.map_err(RegistryServerError::AccessManager)
            }
            _ => Err(RegistryServerError::UnexpectedRequest),
        }
    }

    fn handle_request(
        &mut self,
        token: Token,
        data: &[u8],
    ) -> Result<(), RegistryServerError> {
        let message = Message::from_slice(data).inspect_err(|_| {
            error!("data: {:?}", std::str::from_utf8(data).unwrap());
        })?;
        let response = self.handle_message(message, token.0)?;
        self.handle_response(token, response)?;

        // check if we need to grant access to pending request
        let requests: Vec<AccessGranted> =
            self.access_manager.access_granted_rx.try_iter().collect();
        debug!("Handling pending requests: {:?}", requests);
        for req in requests {
            info!(
                "Access granted for proc: {}, key: {}, request type: {:?}, holder: {}",
                req.0, req.1, req.2, req.3
            );
            let (proc_id, key_id, req_type, holder_id) = req;
            let response = match req_type {
                RequestType::Read => {
                    let addr = self.proc_id_to_addr(holder_id)?;
                    RegistryResponse::Holder(key_id, addr)
                }
                RequestType::Write => RegistryResponse::Success(key_id),
                _ => unreachable!(),
            };

            match self.handle_response(Token(proc_id), response) {
                Err(RegistryServerError::InvalidToken(token)) => {
                    error!(
                        "Invalid token {:?} while handling proc {:?}",
                        token.0, proc_id
                    );
                }
                Err(err) => return Err(err),
                _ => {}
            };
        }

        Ok(())
    }

    fn handle_response(
        &mut self,
        token: Token,
        response: RegistryResponse,
    ) -> Result<(), RegistryServerError> {
        let Some(stream) = self.token_stream_map.get_mut(&token) else {
            return Err(RegistryServerError::InvalidToken(token));
        };
        let data = registry_response!(response).to_vec()?;
        stream.write_all(&data)?;
        Ok(())
    }

    pub fn new(port: u16) -> Result<Self, RegistryServerError> {
        Ok(Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port),
            access_manager: AccessManager::new(),
            token_stream_map: HashMap::new(),
            token_addr_map: HashMap::new(),
            id_counter: 0,
            poll: Poll::new()?,
        })
    }

    fn proc_id_to_addr(
        &self,
        proc_id: ProcId,
    ) -> Result<SocketAddr, RegistryServerError> {
        let token = Token(proc_id);
        self.token_addr_map
            .get(&token)
            .cloned()
            .ok_or_else(|| RegistryServerError::InvalidToken(token))
    }

    fn remove_client(&mut self, token: Token) {
        self.token_stream_map.remove(&token);
        self.token_addr_map.remove(&token);
    }
}
