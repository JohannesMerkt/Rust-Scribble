use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use serde_json::json;
use std::{error};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::net::{TcpStream};
use std::sync::{Arc, Mutex, mpsc, RwLock};
use std::time::{Duration, Instant};
use x25519_dalek::{PublicKey, ReusableSecret};
use rayon::prelude::*;

use rust_scribble_common::network_common::{NetworkInfo, check_checksum, encrypt_json};


use crate::gamestate::GameState;
use crate::lobby::LobbyState;


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
pub(crate) fn check_send_broadcast_messages(
    net_infos: &Arc<RwLock<Vec<Arc<RwLock<NetworkInfo>>>>>,
    rx: mpsc::Receiver<serde_json::Value>,
) {
    //TODO remove disconnected clients from net_infos
    let remove_clients: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    loop {
        if let Ok(msg) = rx.recv() {
            net_infos.write().unwrap().par_iter_mut().for_each(|net_info| {
                match send_message(net_info, &msg) {
                    Ok(_) => {}
                    Err(_) => {
                        remove_clients.lock().unwrap().push(net_info.read().unwrap().username.clone());
                    }
                }
            });
        }

        if remove_clients.lock().unwrap().len() > 0 {
            let mut net_infos = net_infos.write().unwrap();
            for username in remove_clients.lock().unwrap().iter() {
                let index = net_infos.iter().position(|x| x.read().unwrap().username == *username);
                if let Some(index) = index {
                    net_infos.remove(index);
                }
            }
        }
    }
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

/// Initializes the client for the first time
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// * `game_state` - The current game_state.
/// * `lobby` - The lobby state.
/// 
/// # Returns
/// * bool - True if the client is connected, false if not.
/// 
fn client_initialize(
    net_info: &Arc<RwLock<NetworkInfo>>,
    game_state: &Arc<Mutex<GameState>>,
    lobby_state: &Arc<Mutex<LobbyState>>,
    tx: &mpsc::Sender<serde_json::Value>,
) -> bool {
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
    let shared_secret = net_info.secret_key.as_ref().unwrap().diffie_hellman(&client_public);
    net_info.key = *Key::from_slice(shared_secret.as_bytes());

    {
        let username = net_info.username.clone();
        let mut lobby_state = lobby_state.lock().unwrap();
        lobby_state.add_player(username);
        let _ = tx.send(json!(&*lobby_state));
    }

    true
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
pub(crate) fn handle_client(
    net_info: Arc<RwLock<NetworkInfo>>,
    game_state: Arc<Mutex<GameState>>,
    lobby_state: Arc<Mutex<LobbyState>>,
    tx: mpsc::Sender<serde_json::Value>,
) {
    //TODO handle false case for failure to connect
    let _ = client_initialize(&net_info, &game_state, &lobby_state, &tx);
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
