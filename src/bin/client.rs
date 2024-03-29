use std::{io::Write, net::TcpStream};

fn main() -> std::io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:6969")?;

    stream.write("Coucou".as_bytes())?;

    Ok(())
}
