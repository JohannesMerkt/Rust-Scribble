use chacha20poly1305::Key;
use serde_json::json;
use std::io::{BufRead, BufReader, Error, Read};
use std::sync::{Arc, Mutex, mpsc, RwLock};
use std::time::{Duration, Instant};
use x25519_dalek::PublicKey;
use rayon::prelude::*;

use rust_scribble_common::network_common::*;


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
                match send_message(&mut net_info.write().unwrap(), &msg) {
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
        match send_message(&mut net_info.write().unwrap(), &json!({"kind": "ping"})) {
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
        if let Ok(msg) = read_tcp_message(&mut net_info.write().unwrap()) {
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
