use std::collections::HashMap;
use std::io::{Read, Write};

use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use crate::protocol::{DataMessage, KeyId, Message};

use log::{debug, error, info};
use mio::{
    net::{TcpListener, TcpStream},
    Events, Interest, Poll, Token,
};
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum DataStoreError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Key '{0}' already exist!")]
    KeyAlreadyExists(KeyId),

    #[error("Key '{0}' not exist!")]
    KeyDoesNotExists(KeyId),
}

#[derive(Debug, Default)]
pub struct DataStore {
    data_map: Mutex<HashMap<KeyId, Mutex<String>>>,
}

impl DataStore {
    pub fn new() -> Self {
        Self {
            data_map: Mutex::new(HashMap::new()),
        }
    }

    pub fn create(&self, key: KeyId, data: &str) -> Result<(), DataStoreError> {
        self.data_map
            .lock()
            .expect("Unable to get data_map lock for create")
            .insert(key, Mutex::new(data.to_string()));

        Ok(())
    }

    pub fn write(
        &self,
        key: &KeyId,
        new_data: &str,
    ) -> Result<(), DataStoreError> {
        let data_map = self.data_map.lock().unwrap();
        let data = data_map
            .get(key)
            .ok_or_else(|| DataStoreError::KeyDoesNotExists(*key))?;
        let mut data = data.lock().unwrap();
        *data = new_data.to_string();

        Ok(())
    }

    pub fn delete(&self, key: &KeyId) -> Result<(), DataStoreError> {
        self.data_map
            .lock()
            .expect("Unable to get data_map lock for delete")
            .remove(key);

        Ok(())
    }

    pub fn get(&self, key: &KeyId) -> Result<String, DataStoreError> {
        Ok(self
            .data_map
            .lock()
            .unwrap()
            .get(key)
            .ok_or_else(|| DataStoreError::KeyDoesNotExists(*key))?
            .lock()
            .unwrap()
            .clone())
    }
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum P2PServerError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    #[error("ThreadPool error: {0}")]
    ThreadPoolError(#[from] rayon::ThreadPoolBuildError),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("DataStore error: {0}")]
    DataStore(#[from] DataStoreError),
    #[error("Unexpected request")]
    UnexpectedRequest,
    #[error("Unexpected response")]
    UnexpectedResponse,
    #[error("Invalid {:?}", .0)]
    InvalidToken(Token),
}

#[derive(Debug)]
pub struct P2PServer {
    pub peer_addr: SocketAddr,
    pub data_store: Arc<DataStore>,
    token_stream_map: HashMap<Token, TcpStream>,
    poll: Poll,
    token_counter: usize,
}

impl P2PServer {
    pub fn new(
        data_store: Arc<DataStore>,
        peer_addr: SocketAddr,
    ) -> Result<Self, P2PServerError> {
        Ok(Self {
            peer_addr,
            data_store,
            token_stream_map: HashMap::new(),
            poll: Poll::new()?,
            token_counter: 0,
        })
    }

    pub fn bind(&mut self) -> Result<(), P2PServerError> {
        info!("starting p2p server in background on {:?}", self.peer_addr);

        let mut listener = TcpListener::bind(self.peer_addr)?;
        let mut events = Events::with_capacity(128);

        const LISTENER: Token = Token(0);
        self.poll.registry().register(
            &mut listener,
            LISTENER,
            Interest::READABLE,
        )?;

        loop {
            // Attente d'un événement
            self.poll
                .poll(&mut events, None)
                .expect("Error while polling");

            // Traitement des événements
            for event in events.iter() {
                // Si l'événement est lié au listener
                match event.token() {
                    LISTENER => {
                        loop {
                            // Accepte une nouvelle connexion
                            match listener.accept() {
                                // Si la connexion est acceptée
                                Ok((stream, _addr)) => {
                                    self.handle_connection(stream)?
                                }

                                // Si la connexion n'est pas acceptée
                                Err(ref e)
                                    if e.kind()
                                        == io::ErrorKind::WouldBlock =>
                                {
                                    break;
                                }

                                // Si une erreur survient
                                Err(e) => panic!("Unexpected error: {}", e),
                            }
                        }
                    }
                    token if event.is_readable() => {
                        let res = self.handle_data(token);
                        if let Err(err) = res {
                            error!(
                                "Failed to handle data for token: {}, got: {}",
                                token.0, err
                            )
                        }
                    }
                    _ => (),
                }
            }
        }
    }

    fn handle_data(&mut self, token: Token) -> Result<(), P2PServerError> {
        let stream = self.token_stream_map.get_mut(&token).unwrap();
        let mut buffer = [0; 256];
        let read = stream.read(&mut buffer);

        match read {
            Ok(0) => {
                self.token_stream_map.remove(&token);
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {
                self.token_stream_map.remove(&token);
                Err(P2PServerError::IoError(e))
            }
            Ok(size) => self.handle_request(token, &buffer[..size]),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(P2PServerError::IoError(e)),
        }
    }

    pub fn handle_connection(
        &mut self,
        mut stream: TcpStream,
    ) -> Result<(), P2PServerError> {
        self.token_counter += 1;
        let token = Token(self.token_counter);

        self.poll.registry().register(
            &mut stream,
            token,
            Interest::READABLE,
        )?;

        info!("handling connection from {:?}", stream.peer_addr());
        self.token_stream_map.insert(token, stream);

        Ok(())
    }

    fn handle_message(
        &mut self,
        message: Message,
    ) -> Result<DataMessage, P2PServerError> {
        debug!("Handling message: {:?}", message);
        match message {
            Message::Data(DataMessage::Request { key_id }) => {
                let data = self.data_store.get(&key_id)?;
                Ok(DataMessage::Response { data })
            }
            _ => Err(P2PServerError::UnexpectedRequest),
        }
    }

    fn handle_request(
        &mut self,
        token: Token,
        data: &[u8],
    ) -> Result<(), P2PServerError> {
        let message = Message::from_slice(data).inspect_err(|_| {
            error!("data: {:?}", std::str::from_utf8(data).unwrap());
        })?;

        let response = self.handle_message(message)?;
        let Some(stream) = self.token_stream_map.get_mut(&token) else {
            return Err(P2PServerError::InvalidToken(token));
        };

        let data = Message::Data(response).to_vec()?;
        stream.write_all(&data)?;

        Ok(())
    }
}
