use rand_core::OsRng;
use std::io::{Error, Read, Write};
use std::net::TcpStream;
use std::str;
use x25519_dalek::{EphemeralSecret, PublicKey};

fn generate_keypair() -> (PublicKey, EphemeralSecret) {
    let secret = EphemeralSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

pub fn connect_to_server(ip_addr: &str, port: u16) -> Result<(), Error> {
    let (public_key, secret_key) = generate_keypair();

    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:3000") {
        println!("Connected to the server!");

        let mut buffer = [0; 32];
        let read_size = stream.read(&mut buffer)?;
        let server_key: PublicKey = PublicKey::from(buffer);

        println!("Server Key {:?}", server_key.as_bytes());

        stream.write_all(public_key.as_bytes())?;
        println!("My Key {:?}", public_key.as_bytes());

        let shared_secret = secret_key.diffie_hellman(&server_key);
        println!("Shared Secret: {:?}", shared_secret.as_bytes());
    } else {
        println!("Couldn't connect to server...");
    }
    Ok(())
}
