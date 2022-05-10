use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use json::object;
use rand_core::OsRng;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::str;
use std::thread::sleep;
use std::time::Duration;
use x25519_dalek::{EphemeralSecret, PublicKey};

pub struct NetworkInfo {
    username: String,
    stream: TcpStream,
    shared_secret: Key,
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

    let nonce = Nonce::from_slice(b"Unique nonce");
    let cipher = ChaCha20Poly1305::new(&net_info.shared_secret);

    let ciphertext = cipher
        .encrypt(nonce, json_message.dump().as_bytes())
        .expect("encryption failure!");

    let msg_size = ciphertext.len() + 12;
    //prefix the nonce to the ciphertext
    let _ = net_info.stream.write(&usize::to_le_bytes(msg_size));
    let _ = net_info.stream.write(nonce);
    let _ = net_info.stream.write(&ciphertext);
}

fn handle_message(msg: json::JsonValue) {
    println!("{:?}", msg);
}

fn read_tcp_message(net_info: &mut NetworkInfo) {
    let mut size = [0; 8];
    let _ = net_info.stream.read_exact(&mut size);
    let msg_size: usize = usize::from_le_bytes(size);
    let mut msg_buf = vec![0; msg_size];
    let read_size = net_info.stream.read_exact(&mut msg_buf);

    let cipher = ChaCha20Poly1305::new(&net_info.shared_secret);
    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);

    match read_size {
        Ok(_) => {
            let ciphertext = &msg_buf[12..msg_size];
            let recv_data: String = String::from_utf8(cipher.decrypt(&nonce, ciphertext).unwrap())
                .expect("Invalid UTF-8 sequence");
            let json_message = json::parse(&recv_data).unwrap();
            handle_message(json_message);
        }
        Err(e) => println!("Error: {}", e),
    }
}

pub fn get_game_state(net_info: &mut NetworkInfo) {
    loop {
        read_tcp_message(net_info);
        sleep(Duration::from_millis(500));
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

        let shared_secret = secret_key.diffie_hellman(&server_key);
        let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());

        Ok(NetworkInfo {
            username: username.to_string(),
            stream: tcp_stream,
            shared_secret: key,
        })
    } else {
        Err(Error::new(ErrorKind::Other, "Failed to connect to server"))
    }
}
