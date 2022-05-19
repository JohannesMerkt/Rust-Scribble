use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use serde_json::json;
use std::collections::VecDeque;
use std::error;
use std::io::{self, BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use x25519_dalek::{PublicKey, ReusableSecret};

use crate::gamestate::GameState;

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
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(loopback, 3000);
    let listener = TcpListener::bind(socket).unwrap();
    let global_gs = Arc::new(game_state);

    println!("Listening on {}", socket);

    let global_net_infos: Vec<Arc<Mutex<NetworkInfo>>> = Vec::new();
    let broadcast_queue: Arc<Mutex<VecDeque<serde_json::Value>>> =
        Arc::new(Mutex::new(VecDeque::new()));

    //Spin off a thread to wait for broadcast messages and send them to all clients
    let arc_queue = Arc::clone(&broadcast_queue);
    let arc_net_infos = Arc::new(Mutex::new(global_net_infos));
    let net_infos = Arc::clone(&arc_net_infos);

    thread::spawn(move || check_send_broadcast_messages(&net_infos, &arc_queue));

    loop {
        let (public_key, secret_key) = generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        match tcp_stream.write_all(public_key.as_bytes()) {
            Ok(_) => {
                let net_info = Mutex::new(NetworkInfo {
                    username: "".to_string(),
                    tcp_stream,
                    key: *Key::from_slice(public_key.as_bytes()),
                    secret_key,
                });

                let thread_gs = Arc::clone(&global_gs);
                let arc_net_info = Arc::new(net_info);
                let thread_net_info = Arc::clone(&arc_net_info);

                let thread_broadcast_queue = Arc::clone(&broadcast_queue);
                let thread_net_infos = Arc::clone(&arc_net_infos);

                {
                    let glb_cp_net_info = Arc::clone(&arc_net_info);
                    thread_net_infos.lock().unwrap().push(glb_cp_net_info);
                }

                thread::spawn(move || {
                    handle_client(thread_net_info, thread_broadcast_queue, thread_gs)
                });
            }
            Err(e) => println!("Error sending public key to {}: {}", addr, e),
        }
    }
}

fn handle_message(
    msg: serde_json::Value,
    broadcast_queue: &Arc<Mutex<VecDeque<serde_json::Value>>>,
    game_state: &Arc<Mutex<GameState>>,
) {
    println!("RCV: {:?}", msg);

    if msg["msg_type"].eq("chat_message") {
        add_broadcast_message(msg, broadcast_queue);
    } else if msg["msg_type"].eq("guess") {
        println!("Guess: {}", msg["guess"].as_str().unwrap());
    }
}

fn re_add_broadcast_message(
    msg: serde_json::Value,
    broadcast_queue: &Arc<Mutex<VecDeque<serde_json::Value>>>,
) {
    broadcast_queue.lock().unwrap().push_front(msg);
}

fn add_broadcast_message(
    msg: serde_json::Value,
    broadcast_queue: &Arc<Mutex<VecDeque<serde_json::Value>>>,
) {
    broadcast_queue.lock().unwrap().push_back(msg);
}

fn add_game_state_broadcast(
    game_state: &GameState,
    broadcast_queue: &Arc<Mutex<VecDeque<serde_json::Value>>>,
) {
    add_broadcast_message(json!(&game_state), broadcast_queue);
}

fn check_send_broadcast_messages(
    net_infos: &Arc<Mutex<Vec<Arc<Mutex<NetworkInfo>>>>>,
    broadcast_queue: &Arc<Mutex<VecDeque<serde_json::Value>>>,
) {
    loop {
        match broadcast_queue.lock().unwrap().pop_front() {
            Some(msg) => match net_infos.try_lock() {
                Ok(mut net_infos) => {
                    for net_info in net_infos.iter_mut() {
                        let _ = send_message(&net_info, &msg);
                    }
                }
                Err(_) => {
                    // If we couldn't lock the net_infos, readd the message to the queue
                    re_add_broadcast_message(msg, &broadcast_queue);
                }
            },
            _ => {}
        }
    }
}

fn check_checksum(ciphertext: &[u8], checksum: u32) -> bool {
    let checksum_calc = crc32fast::hash(&ciphertext);
    checksum == checksum_calc
}

fn read_tcp_message(
    net_info: &Arc<Mutex<NetworkInfo>>,
) -> Result<serde_json::Value, Box<dyn error::Error>> {
    let json_message;
    let mut size = [0; 8];
    let msg_size;
    let mut msg_buf;
    let cipher;

    {
        let mut net_info = net_info.lock().unwrap();
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

fn client_disconnected(net_info: &Arc<Mutex<NetworkInfo>>, game_state: &Arc<Mutex<GameState>>) {
    let net_info = net_info.lock().unwrap();
    println!("Client {:?} disconnected", net_info.username);
    let mut game_state = game_state.lock().unwrap();
    game_state.remove_player(net_info.username.to_string());
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

fn send_message(net_info: &Arc<Mutex<NetworkInfo>>, msg: &serde_json::Value) -> Result<(), Error> {
    //Don't send messages generated by user to the user
    match net_info.lock() {
        Ok(mut net_info) => {
            if !net_info.username.eq(&msg["user"]) {
                println!("SND {} to {}", &msg, net_info.username);
                let key = net_info.key.clone();
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
    net_info: Arc<Mutex<NetworkInfo>>,
    broadcast_queue: Arc<Mutex<VecDeque<serde_json::Value>>>,
    game_state: Arc<Mutex<GameState>>,
) {
    {
        let mut net_info = net_info.lock().unwrap();

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
            let mut game_state = game_state.lock().unwrap();
            game_state.add_player(username);
            add_game_state_broadcast(&game_state, &broadcast_queue);
        }
    }

    let mut counter = 30;

    loop {
        match read_tcp_message(&net_info) {
            Ok(msg) => {
                handle_message(msg, &broadcast_queue, &game_state);
            }
            Err(_) => {}
        }

        sleep(Duration::from_millis(1000));

        //TODO: Move to a function
        counter -= 1;
        if counter == 0 {
            let ping_msg = json!({"type": "ping","user": "server"});
            let alive = send_message(&net_info, &ping_msg);
            match alive {
                Ok(_) => {
                    counter = 30;
                }
                Err(_) => {
                    client_disconnected(&net_info, &game_state);
                    break;
                }
            }
        }
    }
}
