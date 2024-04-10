mod server;

use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Read, Write};
use std::net::TcpStream;
use rand::{Rng};
use thiserror::Error;

use soros::protocol::Message;
use crate::server::Server;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Failed to bind server!")]
    FailedToBind(),
    #[error("Key '{0}' already exists!")]
    KeyAlreadyExist(String),
    #[error("Key '{0}' doesn't exists!")]
    KeyDoesntExist(String),
}

/// Represents an abstract of a client
pub struct Client {
    /// Client name
    name: String,
    /// Client local host
    host: String,
    /// Client local port
    port: u32,
}

impl Client {
    // Creates a new client with an id, a host and a port
    pub fn new(name: String, host: String, port: u32) -> Self {
        Self {
            name,
            host,
            port,
        }
    }
}

struct ClientState {
    // Client
    client: Client,

    /// Server
    server: Server,

    /// Received p2p requests
    messages: Vec<Message>,
    /// Data stored locally
    data_map: HashMap<String, String>,

    /// TcpStream
    tcp_stream: TcpStream,
}

impl ClientState {
    pub fn new(client: Client, server: Server) -> Self {
        let host_port = format!("{}:{}", server.host, server.port);
        Self {
            client,
            server,

            messages: Vec::new(),
            data_map: HashMap::new(),

            tcp_stream: TcpStream::connect(host_port)
                .expect(ClientError::FailedToBind().to_string().as_str()),
        }
    }

    pub fn create_data(&mut self, key: String, value: String) -> Result<(), ClientError> {
        // Check for existing key
        if self.data_map.contains_key(&key) {
            return Err(ClientError::KeyAlreadyExist(key));
        }

        // Store data
        self.data_map.insert(key.clone(), value);

        Ok(())
    }

    pub fn read_data(&self, key: String) -> Result<String, ClientError> {
        // Check for existing key
        if !self.data_map.contains_key(&key) {
            return Err(ClientError::KeyDoesntExist(key));
        }

        // Extract data to read
        let value = self.data_map.get(&key);

        Ok(String::from(value.unwrap()))
    }
}

// fn client() {
//     // Écoute sur la socket TCP
//     // let mut stream =
//     // TcpStream::connect("127.0.0.1:6969").expect("Error: Faied to bind!");
//     let server = Server::new("127.0.0.1", 6969);
//     let mut stream = server.connect();
//
//     // Envoie de la requête
//     let request = "Coucou, tu veux voir ma... ?";
//     stream.write_all(request.as_bytes()).unwrap();
//     stream.flush().expect("could not flush\n");
//
//     // Réception de la réponse
//     let mut answer = String::new();
//     stream.read_to_string(&mut answer).unwrap();
//
//     println!("Received: {}", answer);
// }
//
// fn main() {
//     client();
// }

fn main() {}