use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{Result, Value};
use std::io::{self, BufRead, BufReader, Error, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::gamestate::GameState;

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

pub fn tcp_server(game_state: Mutex<GameState>) {
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(loopback, 3000);
    let listener = TcpListener::bind(socket).unwrap();
    let global_gs = Arc::new(game_state);
    //let mut global_net_infos = Vec::new();

    println!("Listening on {}, access this port to end the program", 3000);

    loop {
        let (public_key, secret_key) = generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        match tcp_stream.write_all(public_key.as_bytes()) {
            Ok(_) => {
                let thread_gs = Arc::clone(&global_gs);
                thread::spawn(move || handle_client(tcp_stream, secret_key, thread_gs));
            }
            Err(e) => println!("Error sending public key to {}: {}", addr, e),
        }
    }
}

fn handle_message(msg: json::JsonValue, game_state: &Arc<Mutex<GameState>>) {
    //TODO Detect message type and handle accordingly
    println!("{:?}", msg);
}

fn read_tcp_message(net_info: &mut NetworkInfo, game_state: &Arc<Mutex<GameState>>) {
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
            let json_message = json::parse(&recv_data).unwrap();
            handle_message(json_message, &game_state);
        }
        Err(e) => println!("Error: {}", e),
    }
}

fn encrypt_json(json_message: Vec<u8>, shared_key: Key) -> (usize, Nonce, Vec<u8>) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let cipher = ChaCha20Poly1305::new(&shared_key);

    let ciphertext = cipher
        .encrypt(&nonce, &json_message[..])
        .expect("encryption failure!");

    let msg_size = ciphertext.len() + 12;

    (msg_size, nonce, ciphertext)
}

fn client_disconnected(net_info: &mut NetworkInfo, game_state: &Arc<Mutex<GameState>>) {
    println!("Client {:?} disconnected", net_info.username);
    let mut game_state = game_state.lock().unwrap();
    game_state.remove_player(net_info.username.to_string());
}

fn send_tcp_message(
    tcp_stream: &mut TcpStream,
    net_msg: (usize, Nonce, Vec<u8>),
) -> io::Result<()> {
    //TODO send 1 message not 3
    let res = tcp_stream.write(&usize::to_le_bytes(net_msg.0));
    let _ = tcp_stream.write(&net_msg.1);
    let _ = tcp_stream.write(&net_msg.2);

    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

fn send_game_state(
    net_info: &mut NetworkInfo,
    game_state: &Arc<Mutex<GameState>>,
) -> io::Result<()> {
    //TODO get shared game state (don't create it here)

    let new_state = game_state.lock().unwrap();
    let json_gs = serde_json::to_vec(&*new_state).expect("Failed to serialize game state");
    let net_msg = encrypt_json(json_gs, net_info.key);

    send_tcp_message(&mut net_info.tcp_stream, net_msg)
}

fn handle_client(
    mut tcp_stream: TcpStream,
    secret_key: EphemeralSecret,
    game_state: Arc<Mutex<GameState>>,
) {
    //TODO unsafe code, don't depend on fixed length buffers
    let mut buffer = [0; 32];
    let _ = tcp_stream.read(&mut buffer);

    //TODO First message should be a user initialization message
    let mut conn = BufReader::new(&tcp_stream);
    let mut username = String::new();
    let _ = conn.read_line(&mut username);

    let client_public: PublicKey = PublicKey::from(buffer);
    let shared_secret = secret_key.diffie_hellman(&client_public);
    let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());

    let mut net_info = NetworkInfo {
        username,
        tcp_stream,
        key,
    };

    {
        let username = net_info.username.clone();
        let mut game_state = game_state.lock().unwrap();
        game_state.add_player(username);
    }

    loop {
        //TODO make async/await instead of polling
        let res = send_game_state(&mut net_info, &game_state);
        read_tcp_message(&mut net_info, &game_state);
        if res.is_err() {
            client_disconnected(&mut net_info, &game_state);
            break;
        }
        //print the gamestate
        println!("{:?}", game_state.lock().unwrap().to_string());
        sleep(Duration::from_millis(500));
    }
}
