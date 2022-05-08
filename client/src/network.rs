use json::object;
use rand_core::OsRng;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::str;
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

pub struct NetworkInfo {
    username: String,
    stream: TcpStream,
    shared_secret: SharedSecret,
}

fn generate_keypair() -> (PublicKey, EphemeralSecret) {
    let secret = EphemeralSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

// Take message and assemble a json object to send to the server.
pub fn send_chat_message(mut net_info: NetworkInfo, msg: &str) -> Result<usize, Error> {
    let json_message = object! {
        code: 100,
        payload: {
            username: net_info.username,
            message: "Did you see that?"
        }
    };

    net_info.stream.write(json_message.dump().as_bytes())
}

pub fn connect_to_server(ip_addr: &str, port: u16, username: &str) -> Result<NetworkInfo, Error> {
    let (public_key, secret_key) = generate_keypair();

    if let Ok(mut tcp_stream) = TcpStream::connect("127.0.0.1:3000") {
        println!("Connected to the server!");

        let mut buffer = [0; 32];
        let read_size = tcp_stream.read(&mut buffer)?;
        let server_key: PublicKey = PublicKey::from(buffer);

        println!("Server Key {:?}", server_key.as_bytes());

        tcp_stream.write_all(public_key.as_bytes())?;
        println!("My Key {:?}", public_key.as_bytes());

        let shared_secret = secret_key.diffie_hellman(&server_key);
        println!("Shared Secret: {:?}", shared_secret.as_bytes());

        Ok(NetworkInfo {
            username: username.to_string(),
            stream: tcp_stream,
            shared_secret: shared_secret,
        })
    } else {
        Err(Error::new(ErrorKind::Other, "Failed to connect to server"))
    }
}
