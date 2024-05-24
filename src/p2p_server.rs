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
}

#[derive(Debug)]
pub struct P2PServer {
    pub peer_addr: SocketAddr,
    pub data_store: Arc<DataStore>,
}

impl P2PServer {

    pub fn retrieve(
        peer_addr: SocketAddr,
        key_id: KeyId,
    ) -> Result<(), P2PServerError> {
        info!("Getting data from {:?}", peer_addr);

        let mut stream = std::net::TcpStream::connect(peer_addr)?;
        let message = Message::Data(DataMessage::Request { key_id });
        let data = message.to_vec()?;
        stream.write_all(&data)?;

        let mut buffer = [0; 128];
        let read = stream.read(&mut buffer)?;

        let message = Message::from_slice(&buffer[..read])?;
        info!("Data received: {:?}", message);
        match message {
            Message::Data(DataMessage::Response { data }) => {
                info!("Data received: {}", data);
                Ok(())
            }
            _ => Err(P2PServerError::UnexpectedResponse),
        }?;
        Ok(())
    }

    pub fn new(
        data_store: Arc<DataStore>,
        peer_addr: SocketAddr,
    ) -> Result<Self, P2PServerError> {
        info!("Trying to create new p2p server {:?}", peer_addr);

        Ok(Self {
            peer_addr,
            data_store,
        })
    }

    pub fn bind(&mut self) -> Result<(), P2PServerError> {
        info!("Starting p2p server in background on {:?}", self.peer_addr);

        let mut listener = TcpListener::bind(self.peer_addr)?;
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
                                self.handle_connection(stream)
                                    .expect("Error while handling connection");
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

    pub fn handle_connection(
        &mut self,
        mut stream: TcpStream,
    ) -> Result<(), P2PServerError> {
        info!("Handling connection from {:?}", stream.peer_addr());

        let mut buffer = [0; 256];
        let read = stream.read(&mut buffer);

        match read {
            Ok(0) => Ok(()),
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
                let data = self.data_store.get(&key_id)?;
                Ok(DataMessage::Response { data })
            }
            _ => Err(P2PServerError::UnexpectedRequest),
        }
    }

    fn handle_request(&mut self, data: &[u8]) -> Result<(), P2PServerError> {
        let message = Message::from_slice(data).inspect_err(|_| {
            error!("data: {:?}", std::str::from_utf8(data).unwrap());
        })?;
        let _response = self.handle_message(message)?;

        Ok(())
    }
}
