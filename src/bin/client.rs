mod server;

use std::io::{Read, Write};

use server::Server;

fn client() {
    // Écoute sur la socket TCP
    // let mut stream =
    // TcpStream::connect("127.0.0.1:6969").expect("Error: Faied to bind!");
    let server = Server::new("127.0.0.1", 6969);
    let mut stream = server.connect();

    // Envoie de la requête
    let request = "Coucou, tu veux voir ma... ?";
    stream.write_all(request.as_bytes()).unwrap();
    stream.flush().expect("could not flush\n");

    // Réception de la réponse
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();

    println!("Received: {}", answer);
}

fn main() {
    client();
}
