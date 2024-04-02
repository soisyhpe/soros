use std::{
    io::{Read, Write},
    net::TcpStream,
};

fn client() {
    // Écoute sur la socket TCP
    let mut stream = TcpStream::connect("127.0.0.1:6969").expect("Error: Faied to bind!");

    // Envoie de la requête
    let request = "Coucou, tu veux voir ma... ?";
    stream.write(request.as_bytes()).unwrap();

    // Réception de la réponse
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();

    println!("Received: {}", answer);
}

fn main() {
    client();
}
