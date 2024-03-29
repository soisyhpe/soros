use std::{
    io::{self, BufRead, BufReader, Read},
    net::{TcpListener, TcpStream},
};

struct Server {}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6969").unwrap();

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
    Ok(())
}
