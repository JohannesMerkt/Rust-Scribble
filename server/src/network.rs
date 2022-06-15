use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use serde_json::json;
use std::{error};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream, Shutdown};
use std::sync::{Arc, Mutex, mpsc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use x25519_dalek::{PublicKey, ReusableSecret};
use rayon::prelude::*;
use std::sync::mpsc::channel;

use crate::gamestate::GameState;
use crate::lobby::LobbyState;

/// Contains all the information about a client connection.
pub struct NetworkInfo {
    /// The name of the client.
    username: String,
    /// The tcp_stream of the client.
    tcp_stream: TcpStream,
    /// The public key of the client.
    key: Key,
    /// The shared secret of the client and server.
    secret_key: ReusableSecret,
}


/// Runs the listining server for incoming connections.
/// Starts a new thread for each incoming connection
///
/// # Arguments
/// * `game_state` - The game state to be updated.
/// * `port` - The port to listen on.
///
pub fn tcp_server(game_state: Mutex<GameState>, port: u16) {
    let loopback = Ipv4Addr::new(0, 0, 0, 0);
    let socket = SocketAddrV4::new(loopback, port);
    let listener = TcpListener::bind(socket).unwrap();

    let global_gs = Arc::new(game_state);
    let global_lobby = Arc::new(Mutex::new(LobbyState::new()));

    println!("Listening on {}", socket);
    let (tx, rx) = channel();

    //Spin off a thread to wait for broadcast messages and send them to all clients
    let arc_net_infos = Arc::new(RwLock::new(Vec::new()));

    {
        let net_infos = Arc::clone(&arc_net_infos);
        thread::spawn(move || check_send_broadcast_messages(&net_infos, rx));
    }

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

/// Generates a new Public Private keypair.
/// 
/// # Returns
/// * `public_key` - A public key.
/// * `secret_key` - A secret key.
/// 
fn generate_keypair() -> (PublicKey, ReusableSecret) {
    let secret = ReusableSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

/// Handles a client message.
/// 
/// # Arguments
/// * `msg` - The message to be handled in JSON format.
/// * `game_state` - The current game_state which can be updated if necessary.
/// * `lobby_state` - The lobby state which can be updated if necessary.
/// * `tx` - The channel to send broadcast messages to, that will then send to all clients.
/// 
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
    } else if msg["kind"].eq("add_line") {
        let _ = tx.send(msg);
    }
}

/// Loop listening for waiting on MPSC channel and handle sending broadcast messages
/// This function will run in a separate thread.
/// 
/// # Arguments
/// * `net_infos` - Vector of all the network information of each client.
/// * `rx` - The channel to receive broadcast messages from.
/// 
fn check_send_broadcast_messages(
    net_infos: &Arc<RwLock<Vec<Arc<RwLock<NetworkInfo>>>>>,
    rx: mpsc::Receiver<serde_json::Value>,
) {
    //TODO remove disconnected clients from net_infos
    loop {
        if let Ok(msg) = rx.recv() {
            net_infos.write().unwrap().par_iter_mut().for_each(|net_info| {
                let _ = send_message(net_info, &msg);
            });
        }
    }
}

/// Verifies if the checksum of the chipher text is correct.
/// 
/// # Arguments
/// * `cipher_text` - The cipher text to be verified.
/// * `checksum` - The checksum to be verified.
/// 
fn check_checksum(ciphertext: &[u8], checksum: u32) -> bool {
    checksum == crc32fast::hash(ciphertext)
}

/// Reads a tcp_message from the client.
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// 
/// # Returns
/// * `Ok(msg)` - The message read from the client in JSON format.
/// * `Err(e)` - The error that occured.
/// 
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
        Ok(plaintext) => serde_json::from_slice(&plaintext)?,
        Err(_) => return Err(Box::new(Error::new(ErrorKind::Other, "Decryption failed!"))),
    };

    Ok(json_message)

}


/// Encrypts a JSON message
/// 
/// # Arguments
/// * `json_message` - The message to be encrypted.
/// * `share_key` - The shared key to be used for encryption.
/// 
/// # Returns
/// * `(msg_size, nonce,  ciphermsg, checksum)` - A tuple with the size of the whole message(inclusive nonce, checksum, and message), nonce, the encrypted message and the checksum.
///
fn encrypt_json(json_message: Vec<u8>, shared_key: Key) -> (usize, Nonce, Vec<u8>, u32) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let ciphertext = ChaCha20Poly1305::new(&shared_key).encrypt(&nonce, &json_message[..]).expect("encryption failure!");
    let checksum = crc32fast::hash(&ciphertext);

    //Add 12 bytes for the nonce and 4 bytes for the checksum
    let msg_size = ciphertext.len() + 16;
    (msg_size, nonce, ciphertext, checksum)
}

/// Removes a disconnected client from the lobby, gamestate and closes the tcp_stream.
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// * `game_state` - The current game_state.
/// * `lobby` - The lobby state.
/// 
fn client_disconnected(net_info: &Arc<RwLock<NetworkInfo>>, game_state: &Arc<Mutex<GameState>>, lobby: &Arc<Mutex<LobbyState>>) {
    let net_info = net_info.read().unwrap();
    println!("Client {:?} disconnected", net_info.username);
    let mut game_state = game_state.lock().unwrap();
    game_state.remove_player(net_info.username.to_string());
    let mut lobby = lobby.lock().unwrap();
    lobby.remove_player(net_info.username.to_string());
    let _ = net_info.tcp_stream.shutdown(Shutdown::Both);
}

/// Sends a message to a client.
/// 
/// # Arguments
/// * `tcp_stream` - The tcp_stream of the client.
/// * `net_msg: (usize, Nonce, Vec<u8>, u32)` - The prepared encrypted tuple from encrypt_json() to be sent to the client
/// 
/// # Returns
/// * `Ok(())` - The message was sent successfully.
/// * `Err(e)` - The error that occured.
/// 
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

/// Send a JSON message to a client.
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// * `msg` - The message to be sent.
/// 
/// # Returns
/// * `Ok(())` - This function is always successful.
/// 
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

/// Send a JSON message to check if the client is still connected.
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// *`time_elapsed` - The time since the last ping.
/// 
/// # Returns
/// * `Some(bool)` - True if the client is still connected, false if not.
/// * `None` - There was no ping sent.
/// 
fn send_ping_message(net_info: &Arc<RwLock<NetworkInfo>>, time_elapsed: Duration) -> Option<bool> {
    if time_elapsed.as_secs() > 30 {
        match send_message(net_info, &json!({"kind": "ping"})) {
            Ok(_) => Some(true),
            Err(_) => Some(false)
        }
    } else {
        None
    }
}

/// The Main loop to handle each individual clients
/// 
/// This function is should be run in a separate thread.
/// This function reads in the username and create the 
/// shared secret for the client and server to communicate
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// * `game_state` - The current game_state.
/// * `lobby_state` - The lobby state.
/// * `tx` - The channel to send messages to the broadcase thread.
/// 
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

    let mut keepalive = Instant::now();

    //Start of the main loop to read messages and send keepalive pings
    loop {
        if let Ok(msg) = read_tcp_message(&net_info) {
            handle_message(msg, &game_state, &lobby_state, &tx);
            keepalive = Instant::now();
        }

        match send_ping_message(&net_info, Instant::now().duration_since(keepalive)) {
            Some(false) => {
                client_disconnected(&net_info, &game_state, &lobby_state);
                break;
            },
            Some(true) => keepalive = Instant::now(),
            None => {},
        }
        
    }
}
