use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use serde_json::json;
use std::error;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc, RwLock};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use x25519_dalek::{PublicKey, ReusableSecret};
use rayon::prelude::*;
use std::sync::mpsc::channel;

use crate::gamestate::GameState;
use crate::lobby::LobbyState;

pub struct NetworkInfo {
    username: String,
    tcp_stream: TcpStream,
    key: Key,
    secret_key: ReusableSecret,
}

fn generate_keypair() -> (PublicKey, ReusableSecret) {
    let secret = ReusableSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

pub fn tcp_server(game_state: Mutex<GameState>) {
    let loopback = Ipv4Addr::new(0, 0, 0, 0);
    let socket = SocketAddrV4::new(loopback, 3000);
    let listener = TcpListener::bind(socket).unwrap();

    let global_gs = Arc::new(game_state);
    let global_lobby = Arc::new(Mutex::new(LobbyState::new()));

    println!("Listening on {}", socket);

    let global_net_infos: Vec<Arc<RwLock<NetworkInfo>>> = Vec::new();
    let (tx, rx) = channel();

    //Spin off a thread to wait for broadcast messages and send them to all clients
    let arc_net_infos = Arc::new(RwLock::new(global_net_infos));
    let net_infos = Arc::clone(&arc_net_infos);

    thread::spawn(move || check_send_broadcast_messages(&net_infos, rx));

    loop {
        let (public_key, secret_key) = generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        match tcp_stream.write_all(public_key.as_bytes()) {
            Ok(_) => {
                let net_info = RwLock::new(NetworkInfo {
                    username: "".to_string(),
                    tcp_stream,
                    key: *Key::from_slice(public_key.as_bytes()),
                    secret_key,
                });

                let thread_gs = Arc::clone(&global_gs);
                let thread_lobby = Arc::clone(&global_lobby);
                let arc_net_info = Arc::new(net_info);
                let thread_net_info = Arc::clone(&arc_net_info);
                let thread_tx = tx.clone();

                {
                    let thread_net_infos = Arc::clone(&arc_net_infos);
                    let glb_cp_net_info = Arc::clone(&arc_net_info);
                    thread_net_infos.write().unwrap().push(glb_cp_net_info);
                }

                thread::spawn(move || {
                    handle_client(thread_net_info, thread_gs, thread_lobby, thread_tx);
                });
            }
            Err(e) => println!("Error sending public key to {}: {}", addr, e),
        }
    }
}

fn handle_message(
    msg: serde_json::Value,
    game_state: &Arc<Mutex<GameState>>,
    lobby: &Arc<Mutex<LobbyState>>,
    tx: &mpsc::Sender<serde_json::Value>,
) {
    println!("RCV: {:?}", msg);

    if msg["kind"].eq("chat_message") {
        let  _ = tx.send(msg);
    } else if msg["kind"].eq("ready") {
        let mut lobby = lobby.lock().unwrap();
        lobby.set_ready(msg["username"].to_string(), msg["ready"].as_bool().unwrap());
        let _ = tx.send(json!(&*lobby));
    }
}

fn check_send_broadcast_messages(
    net_infos: &Arc<RwLock<Vec<Arc<RwLock<NetworkInfo>>>>>,
    rx: mpsc::Receiver<serde_json::Value>,
) {

    loop {
        if let Ok(msg) = rx.recv() {
            net_infos.write().unwrap().par_iter_mut().for_each(|net_info| {
                let _ = send_message(net_info, &msg);
            });
        }
    }
}

fn check_checksum(ciphertext: &[u8], checksum: u32) -> bool {
    let checksum_calc = crc32fast::hash(ciphertext);
    checksum == checksum_calc
}

fn read_tcp_message(
    net_info: &Arc<RwLock<NetworkInfo>>,
) -> Result<serde_json::Value, Box<dyn error::Error>> {

    let mut size = [0; (usize::BITS / 8) as usize];
    let msg_size;
    let mut msg_buf;
    let cipher;

    {
        let mut net_info = net_info.write().unwrap();
        net_info.tcp_stream.read_exact(&mut size)?;
        msg_size = usize::from_le_bytes(size);

        msg_buf = vec![0; msg_size];
        net_info.tcp_stream.read_exact(&mut msg_buf)?;

        cipher = ChaCha20Poly1305::new(&net_info.key);
    }

    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);
    let ciphertext = &msg_buf[12..msg_size - 4];
    let checksum: u32 = u32::from_le_bytes(msg_buf[msg_size - 4..msg_size].try_into()?);

    //if check_checksum of ciphertext returns false, throw error
    if !check_checksum(ciphertext, checksum) {
        return Err(Box::new(Error::new(
            ErrorKind::InvalidData,
            "Checksum failed",
        )));
    }

    let json_message = match cipher.decrypt(&nonce, ciphertext) {
        Ok(plaintext) => {
            serde_json::from_slice(&plaintext)?
        }
        Err(_) => {
            println!("Decryption failed!");
            return Err(Box::new(Error::new(ErrorKind::Other, "Decryption failed!")));
        }
    };

    Ok(json_message)

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

fn client_disconnected(net_info: &Arc<RwLock<NetworkInfo>>, game_state: &Arc<Mutex<GameState>>, lobby: &Arc<Mutex<LobbyState>>) {
    let net_info = net_info.read().unwrap();
    println!("Client {:?} disconnected", net_info.username);
    let mut game_state = game_state.lock().unwrap();
    game_state.remove_player(net_info.username.to_string());
    let mut lobby = lobby.lock().unwrap();
    lobby.remove_player(net_info.username.to_string());
}

fn send_tcp_message(
    tcp_stream: &mut TcpStream,
    net_msg: (usize, Nonce, Vec<u8>, u32),
) -> Result<(), Error> {
    //TODO send 1 message not 3
    tcp_stream.write_all(&usize::to_le_bytes(net_msg.0))?;
    tcp_stream.write_all(&net_msg.1)?;
    tcp_stream.write_all(&net_msg.2)?;
    tcp_stream.write_all(&u32::to_le_bytes(net_msg.3))?;
    Ok(())
}

fn send_message(net_info: &Arc<RwLock<NetworkInfo>>, msg: &serde_json::Value) -> Result<(), Error> {
    //Don't send messages generated by user to the user
    match net_info.write() {
        Ok(mut net_info) => {
            if !net_info.username.eq(&msg["user"]) {
                println!("SND {} to {}", &msg, net_info.username);
                let key = net_info.key;
                send_tcp_message(
                    &mut net_info.tcp_stream,
                    encrypt_json(msg.to_string().into_bytes(), key),
                )?;
            }
            Ok(())
        }
        Err(_) => Ok(()),
    }
}

fn handle_client(
    net_info: Arc<RwLock<NetworkInfo>>,
    game_state: Arc<Mutex<GameState>>,
    lobby_state: Arc<Mutex<LobbyState>>,
    tx: mpsc::Sender<serde_json::Value>,
) {
    {
        let mut net_info = net_info.write().unwrap();

        let _ = net_info
            .tcp_stream
            .set_read_timeout(Some(Duration::from_millis(50)));

        let mut buffer = [0; 32];
        let _ = net_info.tcp_stream.read(&mut buffer);

        //TODO First message should be a user initialization message
        let mut conn = BufReader::new(&net_info.tcp_stream);
        let mut username = String::new();
        let _ = conn.read_line(&mut username);

        net_info.username = username.trim().to_string();

        let client_public: PublicKey = PublicKey::from(buffer);
        let shared_secret = net_info.secret_key.diffie_hellman(&client_public);
        net_info.key = *Key::from_slice(shared_secret.as_bytes());

        {
            let username = net_info.username.clone();
            let mut lobby_state = lobby_state.lock().unwrap();
            lobby_state.add_player(username);
            let _ = tx.send(json!(&*lobby_state));
        }
    }

    let mut counter = 500;

    loop {
        if let Ok(msg) = read_tcp_message(&net_info) {
            handle_message(msg, &game_state, &lobby_state, &tx);
        }

        sleep(Duration::from_millis(100));

        //TODO: Move to a function
        counter -= 1;
        if counter == 0 {
            let ping_msg = json!({"kind": "ping"});
            let alive = send_message(&net_info, &ping_msg);
            match alive {
                Ok(_) => {
                    counter = 500;
                }
                Err(_) => {
                    client_disconnected(&net_info, &game_state, &lobby_state);
                    break;
                }
            }
        }
    }
}
