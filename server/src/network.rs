use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use json::JsonValue;
use rand::Rng;
use rand_core::OsRng;
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
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

    let mut global_net_infos: Vec<Arc<Mutex<NetworkInfo>>> = Vec::new();
    let broadcast_queue: Arc<Mutex<VecDeque<JsonValue>>> = Arc::new(Mutex::new(VecDeque::new()));

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
    msg: json::JsonValue,
    broadcast_queue: &Arc<Mutex<VecDeque<JsonValue>>>,
    game_state: &Arc<Mutex<GameState>>,
) {
    //add to broadcast queue
    println!("Msg RCV: {:?}", msg);

    match msg["type"].as_str() {
        Some("chat_message") => add_broadcast_message(msg, broadcast_queue),
        Some("guess") => println!(
            "User {}: guessed {}",
            msg["username"].as_str().unwrap(),
            msg["guess"].as_str().unwrap() //TODO check gamestate
        ),
        _ => println!("{:?}", msg),
    }
}

fn readd_broadcast_message(
    msg: json::JsonValue,
    broadcast_queue: &Arc<Mutex<VecDeque<JsonValue>>>,
) {
    broadcast_queue.lock().unwrap().push_front(msg);
}

fn add_broadcast_message(msg: JsonValue, broadcast_queue: &Arc<Mutex<VecDeque<JsonValue>>>) {
    broadcast_queue.lock().unwrap().push_back(msg);
}

fn check_send_broadcast_messages(
    net_infos: &Arc<Mutex<Vec<Arc<Mutex<NetworkInfo>>>>>,
    broadcast_queue: &Arc<Mutex<VecDeque<JsonValue>>>,
) {
    loop {
        let mut broadcast_msg: Option<JsonValue> = None;
        {
            let mut broadcast_queue = broadcast_queue.lock().unwrap();
            if broadcast_queue.len() > 0 {
                println!("Broadcasting message");
                broadcast_msg = Some(broadcast_queue.pop_front().unwrap());
            }
        }

        match broadcast_msg {
            Some(msg) => {
                println!("Sending broadcast message {:?}", msg);
                match net_infos.try_lock() {
                    Ok(mut net_infos) => {
                        for net_info in net_infos.iter_mut() {
                            match net_info.try_lock() {
                                Ok(mut net_info) => {
                                    let _ = send_message(&mut net_info, &msg);
                                }
                                Err(_) => {
                                    println!("Could not lock net_info");
                                }
                            }
                        }
                    }
                    Err(_) => {
                        readd_broadcast_message(msg, &broadcast_queue);
                    }
                }
            }
            None => {}
        }
    }
}

fn read_tcp_message(net_info: &Arc<Mutex<NetworkInfo>>) -> Option<JsonValue> {
    let mut msg_buf: Vec<u8>;
    let msg_size: usize;
    let cipher;
    let result: Result<(), io::Error>;

    {
        let mut net_info = net_info.lock().unwrap();
        let mut size = [0; 8];
        let res_size = net_info.tcp_stream.read_exact(&mut size);
        match res_size {
            Ok(_) => {
                msg_size = usize::from_le_bytes(size);
            }
            Err(_) => {
                return None;
            }
        }

        msg_buf = vec![0; msg_size];
        result = net_info.tcp_stream.read_exact(&mut msg_buf);

        cipher = ChaCha20Poly1305::new(&net_info.key);
    }

    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);

    match result {
        Ok(_) => {
            let ciphertext = &msg_buf[12..msg_size];
            let recv_data: String = String::from_utf8(cipher.decrypt(&nonce, ciphertext).unwrap())
                .expect("Invalid UTF-8 sequence");
            Some(json::parse(&recv_data).unwrap())
        }
        Err(_) => None,
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

fn send_message(net_info: &mut NetworkInfo, msg: &JsonValue) -> io::Result<()> {
    let net_msg = encrypt_json(msg.to_string().into_bytes(), net_info.key);
    send_tcp_message(&mut net_info.tcp_stream, net_msg)
}

fn send_game_state(
    net_info: &mut NetworkInfo,
    game_state: &Arc<Mutex<GameState>>,
) -> io::Result<()> {
    let new_state = game_state.lock().unwrap();
    let json_gs = serde_json::to_vec(&*new_state).expect("Failed to serialize game state");
    let net_msg = encrypt_json(json_gs, net_info.key);

    send_tcp_message(&mut net_info.tcp_stream, net_msg)
}

fn handle_client(
    net_info: Arc<Mutex<NetworkInfo>>,
    broadcast_queue: Arc<Mutex<VecDeque<JsonValue>>>,
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
        }
    }

    loop {
        //TODO make async/await instead of polling

        {
            let mut net_info = net_info.lock().unwrap();
            let res = send_game_state(&mut net_info, &game_state);
            if res.is_err() {
                client_disconnected(&mut net_info, &game_state);
                break;
            }
        }

        match read_tcp_message(&net_info) {
            Some(msg) => {
                handle_message(msg, &broadcast_queue, &game_state);
            }
            _ => {}
        }

        sleep(Duration::from_millis(500));
    }
}
