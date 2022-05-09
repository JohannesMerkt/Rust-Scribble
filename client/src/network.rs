use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
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
pub fn send_chat_message(mut net_info: NetworkInfo, msg: &str) {
    let json_message = object! {
        code: 100,
        payload: {
            username: net_info.username,
            message: msg
        }
    };

    let key: chacha20poly1305::Key = *Key::from_slice(net_info.shared_secret.as_bytes());
    let nonce = Nonce::from_slice(b"Unique nonce");
    let cipher = ChaCha20Poly1305::new(&key);

    let ciphertext = cipher
        .encrypt(nonce, json_message.dump().as_bytes())
        .expect("encryption failure!");

    let msg_size = ciphertext.len() + 12;
    //prefix the nonce to the ciphertext
    let _ = net_info.stream.write(&usize::to_le_bytes(msg_size));
    let _ = net_info.stream.write(nonce);
    let _ = net_info.stream.write(&ciphertext);
}

pub fn connect_to_server(ip_addr: &str, port: u16, username: &str) -> Result<NetworkInfo, Error> {
    let (public_key, secret_key) = generate_keypair();

    if let Ok(mut tcp_stream) = TcpStream::connect("127.0.0.1:3000") {
        println!("Connected to the server!");

        let mut buffer = [0; 32];
        let _ = tcp_stream.read(&mut buffer)?;
        let server_key: PublicKey = PublicKey::from(buffer);
        tcp_stream.write_all(public_key.as_bytes())?;

        let shared_secret = secret_key.diffie_hellman(&server_key);

        Ok(NetworkInfo {
            username: username.to_string(),
            stream: tcp_stream,
            shared_secret: shared_secret,
        })
    } else {
        Err(Error::new(ErrorKind::Other, "Failed to connect to server"))
    }
}
