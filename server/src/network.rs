use chacha20poly1305::Key;
use rust_scribble_common::messages_common::{DisconnectMessage, GameStateUpdate, PlayersUpdate};
use rust_scribble_common::network_common::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use x25519_dalek::PublicKey;

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
fn handle_message(msg: serde_json::Value, server_state: &mut ServerState) -> Vec<Value> {
    println!("RCV: {:?}", msg);
    let mut msg_to_send: Vec<Value> = vec![];

    let send_update = !msg["kind"].eq("update");

    if msg["kind"].eq("user_init") {
        let id = msg["id"].as_i64().unwrap();
        let name = msg["username"].as_str().unwrap();
        server_state.add_player(id, name.to_string());
        msg_to_send.push(json!(PlayersUpdate::new(
            server_state.players().lock().unwrap().to_vec()
        )));
    } else if msg["kind"].eq("ready") {
        let id = msg["id"].as_i64().unwrap();
        let status = msg["ready"].as_bool().unwrap();
        if server_state.set_ready(id, status) {
            server_state.start_game_on_timer(DELAY_BEFORE_GAME_START);
        }
        msg_to_send.push(json!(PlayersUpdate::new(
            server_state.players().lock().unwrap().to_vec()
        )));
    } else if msg["kind"].eq("chat_message") {
        if server_state.chat_or_guess(
            msg["id"].as_i64().unwrap(),
            &msg["message"].as_str().unwrap().to_string(),
        ) {
            server_state.end_game();
        }
        msg_to_send.push(msg);
    } else if msg["kind"].eq("add_line") {
        msg_to_send.push(msg);
    } else if msg["kind"].eq("disconnect") {
        let id = msg["id"].as_i64().unwrap();
        server_state.remove_player(id);
        msg_to_send.push(json!(PlayersUpdate::new(
            server_state.players().lock().unwrap().to_vec()
        )));
    } else {
        msg_to_send.push(msg);
    }

    if send_update {
        msg_to_send.push(json!(GameStateUpdate::new(
            server_state.game_state().lock().unwrap().clone()
        )));
    }

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
    loop {
        if let Ok(msg) = rx.recv() {
            let msgs_to_send = handle_message(msg, &mut server_state.lock().unwrap());

            for msg in msgs_to_send.iter() {
                let client_txs = server_state.lock().unwrap().client_tx();
                for client in client_txs.iter() {
                    if client.tx.send(msg.clone()).is_err() {
                        server_state.lock().unwrap().remove_player(client.id);
                        server_state.lock().unwrap().remove_client_tx(client);
                    }
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
fn send_ping_message(net_info: &mut NetworkInfo, time_elapsed: Duration) -> Option<bool> {
    if time_elapsed.as_secs() > 15 {
        match send_message(net_info, &json!({"kind": "ping"})) {
            Ok(_) => Some(true),
            Err(_) => Some(false),
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
fn client_initialize(net_info: &mut NetworkInfo, tx: &mpsc::Sender<serde_json::Value>) {
    let _ = net_info
        .tcp_stream
        .set_read_timeout(Some(Duration::from_millis(20)));

    let mut buffer = [0; 32];
    let _ = net_info.tcp_stream.read(&mut buffer);

    let mut conn = BufReader::new(&net_info.tcp_stream);
    let mut username = String::new();
    let _ = conn.read_line(&mut username);

    let username = username.trim().to_string();

    let client_public: PublicKey = PublicKey::from(buffer);
    let shared_secret = net_info
        .secret_key
        .as_ref()
        .unwrap()
        .diffie_hellman(&client_public);
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
    mut net_info: NetworkInfo,
    tx: mpsc::Sender<serde_json::Value>,
    rx: mpsc::Receiver<serde_json::Value>,
) {
    client_initialize(&mut net_info, &tx);
    let mut keepalive = Instant::now();
    let player_id = net_info.id;

    //Start of the main loop to read messages and send keepalive pings
    //TODO ideally this would be done async or something cleaner
    loop {
        if let Ok(msg) = read_tcp_message(&mut net_info) {
            let _ = tx.send(msg);
            keepalive = Instant::now();
        }

        // Check if rx has messages waiting and if yes, send them to the client
        if let Ok(msg) = rx.try_recv() {
            let _ = send_message(&mut net_info, &msg);
        }

        match send_ping_message(&mut net_info, Instant::now().duration_since(keepalive)) {
            Some(false) => {
                let _ = tx.send(json!(DisconnectMessage::new(player_id)));
                return;
            }
            Some(true) => keepalive = Instant::now(),
            None => {}
        }
    }
}
