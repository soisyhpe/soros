use std::{
    io::{self, BufReader, BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
};
use std::collections::HashMap;
use thiserror::Error;
use soros::rw_queue::RwQueue;
use crate::Client;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Key '{0}' bounds another client!")]
    KeyBoundsAnotherClient(String),
}

// Represents an abstract state of a server
pub struct Server {
    // Server name
    pub name: String,
    // Server host
    pub host: String,
    // Server port
    pub port: u32,
}

impl Server {
    pub fn new(name: String, host: String, port: u32) -> Self {
        Self {
            name,
            host,
            port,
        }
    }
}

/// Represents local state of a client
pub struct LocalClient {
    /// Client
    client: Client,
    /// Read/Write queue
    rw_queue: RwQueue,
}

impl LocalClient {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            rw_queue: RwQueue::new(),
        }
    }
}

// Represents the state of a server
pub struct ServerState {
    // Server
    server: Server,
    // Map of stored keys with their clients
    clients: HashMap<String, LocalClient>,
}

impl ServerState {
    pub fn new(host: String, port: u32) -> Self {
        Self {
            server: Server::new(String::from("S"), host, port),
            clients: HashMap::new(),
        }
    }

    /// Write a data
    pub fn write(&self) -> Result<(), ServerError> {
        Ok(())
    }

    /// Read a data
    pub fn read(&self) -> Result<(), ServerError> {
        Ok(())
    }

    pub fn connect(self) -> TcpStream {
        TcpStream::connect(format!("{}:{}", self.host, self.port))
            .expect("Error: Failed to bind!")
    }

    pub fn listen(&mut self) {
        let listener =
            TcpListener::bind(format!("{}:{}", self.host, self.port)).unwrap();
        // Boucle qui permet de gérer plusieurs clients simultanément
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            println!("Connection established!");

            let _ = self.handle_connection(stream).map_err(|err| {
                println!("Error: {:?}", err);
            });
        }
    }

    fn handle_connection(
        &mut self,
        mut stream: TcpStream,
    ) -> Result<(), io::Error> {
        let mut buf_reader = BufReader::new(&mut stream);
        let mut buffer = String::new();

        buf_reader.read_to_string(&mut buffer)?;
        println!("Req: {}", buffer);
        // self.messages.push(buffer);

        let mut buf_writer = BufWriter::new(stream);
        buf_writer.write_all(b"Answer")?;

        println!("Answer sent");
        // println!("messages: {:#?}", self.messages);

        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn server_creation() {
//         let mut server = Server::new("127.0.0.1", 6969);
//         server.listen();

//         let mut client = server.connect();
//         client.write_all("PSAR".as_bytes()).expect("");
//     }
// }

fn main() {
    let mut server = ServerState::new("127.0.0.1", 6969);
    server.listen();

    let mut client = server.connect();
    client.write_all("PSAR".as_bytes()).expect("");
}
