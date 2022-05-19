use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use serde_json::Value;
use std::error;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::str;
use std::time::Duration;
use x25519_dalek::{EphemeralSecret, PublicKey};

pub struct NetworkInfo {
    username: String,
    tcp_stream: TcpStream,
    key: Key,
}

fn generate_keypair() -> (PublicKey, EphemeralSecret) {
    let secret = EphemeralSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

fn encrypt_json(json_message: Vec<u8>, shared_key: Key) -> (usize, Nonce, Vec<u8>, u32) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let cipher = ChaCha20Poly1305::new(&shared_key);

    let ciphertext = cipher
        .encrypt(&nonce, &json_message[..])
        .expect("encryption failure!");

    let checksum = crc32fast::hash(&ciphertext);

    //Add 12 bytes for the nonce and 4 bytes for the checksum
    let msg_size = ciphertext.len() + 16;

    (msg_size, nonce, ciphertext, checksum)
}

fn check_checksum(ciphertext: &[u8], checksum: u32) -> bool {
    let checksum_calc = crc32fast::hash(&ciphertext);
    checksum == checksum_calc
}

fn send_tcp_message(
    tcp_stream: &mut TcpStream,
    net_msg: (usize, Nonce, Vec<u8>, u32),
) -> Result<(), Error> {
    //TODO send 1 message not 3
    tcp_stream.write(&usize::to_le_bytes(net_msg.0))?;
    tcp_stream.write(&net_msg.1)?;
    tcp_stream.write(&net_msg.2)?;
    tcp_stream.write(&u32::to_le_bytes(net_msg.3))?;
    Ok(())
}

// Take message and assemble a json object to send to the server.
pub fn send_message(net_info: &mut NetworkInfo, msg: Value) -> Result<(), Error> {
    send_tcp_message(
        &mut net_info.tcp_stream,
        encrypt_json(msg.to_string().into_bytes(), net_info.key),
    )
}

pub fn read_tcp_message(
    net_info: &mut NetworkInfo,
) -> Result<serde_json::Value, Box<dyn error::Error>> {
    let json_message;
    let mut size = [0; (usize::BITS / 8) as usize];
    net_info.tcp_stream.read_exact(&mut size)?;
    let msg_size: usize = usize::from_le_bytes(size);

    let mut msg_buf = vec![0; msg_size];
    net_info.tcp_stream.read_exact(&mut msg_buf)?;

    let cipher = ChaCha20Poly1305::new(&net_info.key);
    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);

    let ciphertext = &msg_buf[12..msg_size - 4];
    let checksum: u32 = u32::from_le_bytes(msg_buf[msg_size - 4..msg_size].try_into()?);

    //if check_checksum of ciphertext returns false, throw error
    if !check_checksum(&ciphertext, checksum) {
        return Err(Box::new(Error::new(
            ErrorKind::InvalidData,
            "Checksum failed",
        )));
    }

    match cipher.decrypt(&nonce, ciphertext) {
        Ok(plaintext) => {
            json_message = serde_json::from_slice(&plaintext)?;
        }
        Err(_) => {
            println!("Decryption failed!");
            return Err(Box::new(Error::new(ErrorKind::Other, "Decryption failed!")));
        }
    }

    Ok(json_message)
}

pub fn connect_to_server(ip_addr: &str, port: u16, username: &str) -> Result<NetworkInfo, Error> {
    let (public_key, secret_key) = generate_keypair();

    let ip_addr = ip_addr.parse::<std::net::Ipv4Addr>().unwrap();
    let socket = std::net::SocketAddrV4::new(ip_addr, port);
    if let Ok(mut tcp_stream) = TcpStream::connect(socket) {
        println!("Connected to the server!");

        let mut buffer = [0; 32];
        let _ = tcp_stream.read(&mut buffer)?;
        let server_key: PublicKey = PublicKey::from(buffer);
        tcp_stream.write_all(public_key.as_bytes())?;
        tcp_stream.write_all(username.as_bytes())?;

        let _ = tcp_stream.set_read_timeout(Some(Duration::from_millis(50)));

        let shared_secret = secret_key.diffie_hellman(&server_key);
        let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());

        Ok(NetworkInfo {
            username: username.to_string(),
            tcp_stream,
            key,
        })
    } else {
        Err(Error::new(ErrorKind::Other, "Failed to connect to server"))
    }
}
