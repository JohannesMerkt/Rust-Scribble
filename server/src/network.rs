use chacha20poly1305::Key;
use rust_scribble_common::messages_common::{GameStateUpdate, DisconnectMessage};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex, mpsc, RwLock};
use std::time::{Duration, Instant};
use x25519_dalek::PublicKey;
use rust_scribble_common::network_common::*;

use crate::serverstate::ServerState;

const DELAY_BEFORE_GAME_START: u64 = 5;

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
    server_state: &mut ServerState,
) -> Vec<Value> {

    println!("RCV: {:?}", msg);
    let mut msg_to_send:Vec<Value> = vec![];

    //TODO create message structs and remove unpack and repacking
    if msg["kind"].eq("user_init") {
        let id = msg["id"].as_i64().unwrap();
        let name = msg["username"].as_str().unwrap();
        server_state.add_player(id, name.to_string());
    } else if msg["kind"].eq("ready") {
        let id = msg["id"].as_i64().unwrap();
        let status = msg["ready"].as_bool().unwrap();
        if server_state.set_ready(id, status) {
            server_state.start_game_on_timer(DELAY_BEFORE_GAME_START);
        }
    } else if msg["kind"].eq("chat_message") {
        server_state.chat_or_guess(msg["id"].as_i64().unwrap(), &msg["message"].as_str().unwrap().to_string());
        msg_to_send.push(msg);
    } else if msg["kind"].eq("add_line") {
        msg_to_send.push(msg);
    } else if msg["kind"].eq("disconnect") {
        let id = msg["id"].as_i64().unwrap();
        server_state.remove_player(id);
        msg_to_send.push(msg);
    } else {
        msg_to_send.push(msg);
    }

    msg_to_send.push(json!(GameStateUpdate::new(0, server_state.game_state().lock().unwrap().clone())));

    msg_to_send
}

/// Loop listening for waiting on MPSC channel and handle sending broadcast messages
/// This function will run in a separate thread.
/// 
/// # Arguments
/// * `net_infos` - Vector of all the network information of each client.
/// * `rx` - The channel to receive broadcast messages from.
/// 
pub(crate) fn check_send_broadcast_messages(
    server_state: Arc<Mutex<ServerState>>,
    rx: mpsc::Receiver<serde_json::Value>,
) {
    //TODO remove disconnected clients from net_infos
    let remove_clients: Arc<Mutex<Vec<i64>>> = Arc::new(Mutex::new(Vec::new()));

    loop {
        if let Ok(msg) = rx.recv() {

            let msgs_to_send = handle_message(msg, &mut server_state.lock().unwrap());

            for msg in msgs_to_send.iter() {
                if msg["kind"].eq("disconnect") {
                    let mut remove_clients = remove_clients.lock().unwrap();
                    remove_clients.push(msg["id"].as_i64().unwrap());
                } else {
                    let net_infos = server_state.lock().unwrap().net_infos();
                    net_infos.write().unwrap().iter_mut().for_each(|net_info| {
                        let mut net_info = net_info.write().unwrap();
                        match send_message(&mut net_info, &msg) {
                            Ok(_) => {}
                            Err(_) => {
                                remove_clients.lock().unwrap().push(net_info.id);
                            }
                        }
                    });
                }
            }

            {
                let net_infos = server_state.lock().unwrap().net_infos();
                if remove_clients.lock().unwrap().len() > 0 {
                    for id in remove_clients.lock().unwrap().iter() {
                        let index = net_infos.read().unwrap().iter().position(|x| x.read().unwrap().id == *id);
                        if let Some(index) = index {
                            net_infos.write().unwrap().remove(index);
                        }
                    }
                    remove_clients.lock().unwrap().clear();
                }
            }
        }
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
    if time_elapsed.as_secs() > 15 {
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
    tx: &mpsc::Sender<serde_json::Value>,
) {
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

    let username = username.trim().to_string();

    let client_public: PublicKey = PublicKey::from(buffer);
    let shared_secret = net_info.secret_key.as_ref().unwrap().diffie_hellman(&client_public);
    net_info.key = *Key::from_slice(shared_secret.as_bytes());

    let _ = tx.send(json!({"kind": "user_init", "id": net_info.id , "username": username}));
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
/// * `tx` - The channel to send messages to the broadcast thread.
/// 
pub(crate) fn handle_client(
    net_info: Arc<RwLock<NetworkInfo>>,
    tx: mpsc::Sender<serde_json::Value>,
) {

    client_initialize(&net_info, &tx);
    let mut keepalive = Instant::now();
    let player_id = net_info.read().unwrap().id.clone();

    //Start of the main loop to read messages and send keepalive pings
    //TODO ideally this would be done async or something cleaner
    loop {
        if let Ok(msg) = read_tcp_message(&mut net_info.write().unwrap()) {
            let _ = tx.send(msg);
            keepalive = Instant::now();
        }

        match send_ping_message(&net_info, Instant::now().duration_since(keepalive)) {
            Some(false) => {
                let _ = tx.send(json!(DisconnectMessage::new(player_id)));
                return
            },
            Some(true) => keepalive = Instant::now(),
            None => {},
        }
        
    }
}
