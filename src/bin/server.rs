use std::{
    io::{self, BufReader, BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
};

pub struct Server {
    host: String,
    port: i32,
    messages: Vec<String>,
}

impl Server {
    pub fn new(host: &str, port: i32) -> Self {
        Self {
            host: host.to_string(),
            port,
            messages: Vec::new(),
        }
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
        self.messages.push(buffer);

        let mut buf_writer = BufWriter::new(stream);
        buf_writer.write_all(b"Answer")?;

        println!("Answer sent");
        println!("messages: {:#?}", self.messages);

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
    let mut server = Server::new("127.0.0.1", 6969);
    server.listen();

    let mut client = server.connect();
    client.write_all("PSAR".as_bytes()).expect("");
}
