use std::{
    io::{self, BufReader, BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
};

// struct Server {
//     // strings: [String],
//     strings: Vec<String>,
// }

// impl Server {
//     fn new(strings: Vec<String>) -> Self {
//         Self { strings }
//     }
//     fn listen() {}
//     fn handle_request() {}
// }

fn main() {
    let host = "127.0.0.1";
    let port = "6969";

    let listener = TcpListener::bind(format!("{}:{}", host, port)).unwrap();

    // Boucle qui permet de gérer plusieurs clients simultanément
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");
        let _ = handle_connection(stream).map_err(|err| {
            println!("Error: {:?}", err);
        });
    }
}

fn handle_connection(mut stream: TcpStream) -> Result<(), io::Error> {
    let mut buf_reader = BufReader::new(&mut stream);
    let mut buffer = String::new();
    buf_reader.read_to_string(&mut buffer)?;
    println!("Req: {}", buffer);

    let mut buf_writer = BufWriter::new(stream);
    buf_writer.write(b"Answer")?;

    println!("Answer sent");

    Ok(())
}
