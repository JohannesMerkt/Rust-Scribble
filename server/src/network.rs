use async_std::net::TcpStream as AsyncTcpStream;
use async_std::prelude::*;
use async_std::task::{sleep, spawn};
use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use json::object;
use json::JsonValue;
use rand::Rng;
use rand_core::OsRng;
use std::io::{Error, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::thread;
use x25519_dalek::{EphemeralSecret, PublicKey};

pub struct NetworkInfo {
    username: String,
    tcp_stream: AsyncTcpStream,
    shared_secret: Key,
}

fn generate_keypair() -> (PublicKey, EphemeralSecret) {
    let secret = EphemeralSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

pub fn tcp_server() {
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(loopback, 3000);
    let listener = TcpListener::bind(socket).unwrap();
    println!("Listening on {}, access this port to end the program", 3000);

    loop {
        //TODO accept connection and start a thread to handle it
        let (public_key, secret_key) = generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        match tcp_stream.write_all(public_key.as_bytes()) {
            Ok(_) => {
                thread::spawn(move || handle_client(tcp_stream, secret_key));
            }
            Err(e) => println!("Error sending public key to {}: {}", addr, e),
        }
    }
}

fn encrypt_json(json_message: JsonValue, shared_key: Key) -> (usize, Nonce, Vec<u8>) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let cipher = ChaCha20Poly1305::new(&shared_key);

    let ciphertext = cipher
        .encrypt(&nonce, json_message.dump().as_bytes())
        .expect("encryption failure!");

    let msg_size = ciphertext.len() + 12;

    (msg_size, nonce, ciphertext)
}

async fn read_tcp_message(net_info: &mut NetworkInfo) -> Result<JsonValue, Error> {
    let mut size = [0; 8];
    let _ = net_info.tcp_stream.read_exact(&mut size);
    let msg_size: usize = usize::from_le_bytes(size);
    let mut msg_buf = vec![0; msg_size];
    let read_size = net_info.tcp_stream.read_exact(&mut msg_buf).await;

    let cipher = ChaCha20Poly1305::new(&net_info.shared_secret);
    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);

    match read_size {
        Ok(_) => {
            let ciphertext = &msg_buf[12..msg_size];
            let recv_data: String = String::from_utf8(cipher.decrypt(&nonce, ciphertext).unwrap())
                .expect("Invalid UTF-8 sequence");

            //TODO convert to serde_json
            let json_message = json::parse(&recv_data).unwrap();
            Ok(json_message)
        }
        Err(e) => Err(e),
    }
}

async fn send_tcp_message<'a>(
    tcp_stream: &'a mut AsyncTcpStream,
    net_msg: (usize, Nonce, Vec<u8>),
) {
    //TODO send 1 message not 3
    let _ = tcp_stream.write(&usize::to_le_bytes(net_msg.0));
    let _ = tcp_stream.write(&net_msg.1);
    let _ = tcp_stream.write(&net_msg.2);
}

pub async fn send_game_state<'a>(net_info: &'a mut NetworkInfo) {
    //TODO get shared game state (don't create it here)
    let game_state = object! {
        code: 100,
        payload: {
            users: {
                "Bob": 10,
                "Sally": 0,
                "Alice": 10
            },
            turn: "Bob",
            image:[[10, 5],[11, 5],[12, 5]]
        }
    };

    let net_msg = encrypt_json(game_state, net_info.shared_secret);
    send_tcp_message(&mut net_info.tcp_stream, net_msg).await;
}

async fn check_for_message(net_info: &mut NetworkInfo) {
    let mut buffer = vec![0; 1024];
    let mut result: Result<usize, Error> = Ok(0);

    while result.is_err() || result.unwrap() < 16 {
        result = net_info.tcp_stream.peek(&mut buffer).await;
    }

    let json_messge = read_tcp_message(net_info).await;
    //TODO handle message
    println!("{:?}", json_messge);
}

async fn handle_client(mut tcp_stream: TcpStream, secret_key: EphemeralSecret) {
    let mut buffer = [0; 32];
    let _ = tcp_stream.read(&mut buffer);

    let client_public: PublicKey = PublicKey::from(buffer);
    let shared_secret = secret_key.diffie_hellman(&client_public);
    let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());

    let async_stream = AsyncTcpStream::from(tcp_stream);

    //TODO get username from client
    let mut net_info = NetworkInfo {
        username: "Bob".to_string(),
        tcp_stream: async_stream,
        shared_secret: key,
    };

    loop {
        send_game_state(&mut net_info).await();
        //spawn(check_for_message(&mut net_info));
        sleep(std::time::Duration::from_secs(1)).await;
    }
}
