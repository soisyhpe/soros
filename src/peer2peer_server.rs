use std::{io, net::ToSocketAddrs};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use log::{debug, error, info};
use mio::{
    Events,
    Interest, net::{TcpListener, TcpStream}, Poll, Token,
};
use thiserror::Error;
use crate::protocol::{DataMessage, KeyId, Message};

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum DataStoreError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Key '{0}' already exist!")]
    KeyAlreadyExists(KeyId),
    #[error("Key '{0}' not exist!")]
    KeyDoesNotExists(KeyId),
}

#[derive(Debug)]
pub struct DataStore {
    data_map: HashMap<KeyId, Arc<Mutex<String>>>,
}

impl DataStore {
    pub fn new() -> Self {
        Self {
            data_map: HashMap::new(),
        }
    }

    pub fn create(&mut self, key: KeyId, data: &str) -> Result<(), DataStoreError> {
        if self.data_map.contains_key(&key) {
            return Err(DataStoreError::KeyAlreadyExists(key));
        }

        self.data_map.insert(key, Arc::new(Mutex::new(data.to_string())));

        Ok(())
    }

    pub fn write(&mut self, key: KeyId, new_data: &str) -> Result<(), DataStoreError> {
        let data = self.data_map.get(&key).ok_or(DataStoreError::KeyDoesNotExists(key))?;
        let mut data = data.lock().unwrap();
        *data = new_data.to_string();

        Ok(())
    }

    pub fn delete(&mut self, key: KeyId) -> Result<(), DataStoreError> {
        self.data_map.remove(&key);

        Ok(())
    }

    pub fn get(&self, key: KeyId) -> Result<String, DataStoreError> {
        match self.data_map.get(&key) {
            Some(data) => {
                let data = data.lock().unwrap();
                Ok(data.clone())
            }
            None => Err(DataStoreError::KeyDoesNotExists(key)),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum P2PServerError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("ThreadPool error: {0}")]
    ThreadPoolError(#[from] rayon::ThreadPoolBuildError),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("DataStore error: {0}")]
    DataStore(#[from] DataStoreError),
    #[error("Unexpected request")]
    UnexpectedRequest,
}

#[derive(Debug)]
pub struct P2PServer {
    pub hostname: String,
    pub port: u16,
    pub data_store: DataStore,
    pub thread_pool: rayon::ThreadPool,
}

impl P2PServer {
    pub fn get(&mut self, hostname: &str, port: u32, key_id: KeyId) -> Result<String, P2PServerError> {
        info!("Getting data from {}:{}", hostname, port);

        let mut stream = std::net::TcpStream::connect(format!("{}:{}", hostname, port))?;
        let message = Message::Data(DataMessage::Request { key_id });
        let data = message.to_vec()?;
        stream.write_all(&data)?;

        let mut buffer = [0; 256];
        let read = stream.read(&mut buffer)?;

        let message = Message::from_slice(&buffer[..read])?;
        let response = self.handle_message(message)?;

        match response {
            DataMessage::Response { data } => Ok(data),
            _ => Err(P2PServerError::UnexpectedRequest),
        }
    }

    pub fn new(data_store: DataStore, hostname: &str, port: u16) -> Result<Self, P2PServerError> {
        info!("Trying to create new p2p server {}:{}", hostname, port);

        let thread_pool = rayon::ThreadPoolBuilder::new().num_threads(4).build()?;

        Ok(Self {
            hostname: hostname.to_string(),
            port,
            data_store,
            thread_pool,
        })
    }

    pub fn start_background(&mut self) -> Result<(), P2PServerError> {
        info!("Starting p2p server in background on {}:{}", self.hostname, self.port);

        let addr_string = format!("{}:{}", self.hostname, self.port);
        let addr = addr_string.to_socket_addrs()?.next().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                format!("Could not find address {}", addr_string),
            )
        })?;
        let mut listener = TcpListener::bind(addr)?;

        let mut events = Events::with_capacity(128);

        let mut poll = Poll::new()?;

        const LISTENER: Token = Token(0);
        poll.registry().register(
            &mut listener,
            LISTENER,
            Interest::READABLE,
        )?;

        loop {
            // Attente d'un événement
            poll.poll(&mut events, None).expect("Error while polling");

            // Traitement des événements
            for event in events.iter() {

                // Si l'événement est lié au listener
                match event.token() {
                    LISTENER => loop {

                        // Accepte une nouvelle connexion
                        match listener.accept() {

                            // Si la connexion est acceptée
                            Ok((stream, _addr)) => {
                                // self.thread_pool.install(|| {
                                self.handle_connection(stream).expect("Error while handling connection");
                                // })
                            }

                            // Si la connexion n'est pas acceptée
                            Err(ref e)
                            if e.kind() == io::ErrorKind::WouldBlock =>
                                {
                                    break;
                                }

                            // Si une erreur survient
                            Err(e) => panic!("Unexpected error: {}", e),
                        }
                    },

                    // Si l'événement n'est pas lié au listener, on ne fait rien
                    _ => {}
                }
            }
        }
    }

    pub fn handle_connection(&mut self, mut stream: TcpStream) -> Result<(), P2PServerError> {
        info!("Handling connection from {:?}", stream.peer_addr());

        let mut buffer = [0; 256];
        let read = stream.read(&mut buffer);

        match read {
            Ok(0) => {
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {
                Err(P2PServerError::IoError(e))
            }
            Ok(size) => {
                let _res = self.handle_request(&buffer[..size]);
                Ok(())
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(P2PServerError::IoError(e)),
        }?;

        Ok(())
    }

    fn handle_message(
        &mut self,
        message: Message,
    ) -> Result<DataMessage, P2PServerError> {
        debug!("Handling message: {:?}", message);
        match message {
            Message::Data(DataMessage::Request { key_id }) => {
                let data = self.data_store.get(key_id)?;
                Ok(DataMessage::Response { data })
            }
            _ => Err(P2PServerError::UnexpectedRequest),
        }
    }

    fn handle_request(
        &mut self,
        data: &[u8],
    ) -> Result<(), P2PServerError> {
        let message = Message::from_slice(data).inspect_err(|_| {
            error!("data: {:?}", std::str::from_utf8(data).unwrap());
        })?;
        let _response = self.handle_message(message)?;

        Ok(())
    }
}

