use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use serde_json::{json, Value};
use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::str;
use std::thread::sleep;
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

// Take message and assemble a json object to send to the server.
pub fn send_chat_message(net_info: &mut NetworkInfo, msg: &str) {
    let json_message = json!({
        "code": 100,
        "payload": {
            "user": "Bob",
            "message": msg.to_string(),
        }
    });

    let net_msg = encrypt_json(json_message, net_info.key);
    send_tcp_message(&mut net_info.tcp_stream, net_msg)
}

fn handle_message(msg: serde_json::Value) {
    //TODO Detect message type and handle accordingly
    println!("{:?}", msg);
}

fn encrypt_json(json_message: Value, shared_key: Key) -> (usize, Nonce, Vec<u8>) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let cipher = ChaCha20Poly1305::new(&shared_key);

    let ciphertext = cipher
        .encrypt(
            &nonce,
            serde_json::to_string(&json_message).unwrap().as_bytes(),
        )
        .expect("encryption failure!");

    let msg_size = ciphertext.len() + 12;

    (msg_size, nonce, ciphertext)
}

fn send_tcp_message(tcp_stream: &mut TcpStream, net_msg: (usize, Nonce, Vec<u8>)) {
    //TODO send 1 message not 3
    let _ = tcp_stream.write(&usize::to_le_bytes(net_msg.0));
    let _ = tcp_stream.write(&net_msg.1);
    let _ = tcp_stream.write(&net_msg.2);
}

fn read_tcp_message(net_info: &mut NetworkInfo) {
    let mut size = [0; 8];
    let _ = net_info.tcp_stream.read_exact(&mut size);
    let msg_size: usize = usize::from_le_bytes(size);

    if msg_size == 0 {
        return;
    }

    let mut msg_buf = vec![0; msg_size];
    let read_size = net_info.tcp_stream.read_exact(&mut msg_buf);

    let cipher = ChaCha20Poly1305::new(&net_info.key);
    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);

    match read_size {
        Ok(_) => {
            let ciphertext = &msg_buf[12..msg_size];
            let recv_data: String = String::from_utf8(cipher.decrypt(&nonce, ciphertext).unwrap())
                .expect("Invalid UTF-8 sequence");
            let json_message = serde_json::from_str(&recv_data).unwrap();
            handle_message(json_message);
        }
        Err(e) => println!("Error: {}", e),
    }
}

pub fn get_game_state(net_info: &mut NetworkInfo) {
    loop {
        read_tcp_message(net_info);
        sleep(Duration::from_millis(500));
        send_chat_message(net_info, "Test Test message");
    }
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
