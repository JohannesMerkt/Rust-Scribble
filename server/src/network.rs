use std::io::{BufRead, BufReader, Read};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use chacha20poly1305::Key;
use rust_scribble_common::messages_common::{
    ChatMessage, DisconnectMessage, GameStateUpdate, PlayersUpdate,
};
use rust_scribble_common::network_common::*;
use serde_json::{json, Value};
use x25519_dalek::PublicKey;

use crate::lobbystate;
use crate::lobbystate::{GuessResult, LobbyState};

const DELAY_BEFORE_GAME_START: u64 = 3;
// seconds
const MIN_TIME_BETWEEN_PINGS: u64 = 15; // seconds

/// Handles a client message.
///
/// # Arguments
/// * `msg` - The message to be handled in JSON format.
/// * `lobby` - The lobby state which can be updated if necessary when processing the message.
///
/// # Returns
/// * `Vector<Value>` - A vector of JSON value messages that shall be sent to all clients of the lobby.
///
fn handle_message(msg: Value, lobby: &mut LobbyState) -> Vec<Value> {
    let mut msg_to_send: Vec<Value> = vec![];

    // kind 'update_requested' will automatically trigger send_update = true
    // and does not need to be handled explicitly
    let send_update = !msg["kind"].eq("update");
    let mut clean_up_lobby = false;

    if msg["kind"].eq("user_init") {
        let id = msg["id"].as_i64().unwrap();
        let name = msg["username"].as_str().unwrap();
        lobby.add_player(id, name.to_string());
        msg_to_send.push(json!(PlayersUpdate::new(
            lobby.players().lock().unwrap().to_vec()
        )));
    } else if msg["kind"].eq("ready") {
        let id = msg["id"].as_i64().unwrap();
        let status = msg["ready"].as_bool().unwrap();
        lobby.set_ready(id, status);
        if lobby.all_ready()
            && lobby.players().lock().unwrap().len() >= lobbystate::MIN_NUMBER_PLAYERS
        {
            lobby.start_game_on_timer(DELAY_BEFORE_GAME_START);
        }
        msg_to_send.push(json!(PlayersUpdate::new(
            lobby.players().lock().unwrap().to_vec()
        )));
    } else if msg["kind"].eq("chat_message") {
        let guess_result = lobby.chat_or_correct_guess(
            msg["id"].as_i64().unwrap(),
            msg["message"].as_str().unwrap(),
        );
        match guess_result {
            GuessResult::Correct => {
                if lobby.all_guessed() {
                    clean_up_lobby = true;
                }
                msg_to_send.push(json!(ChatMessage::new(
                    msg["id"].as_i64().unwrap(),
                    "Guessed the word correctly!".to_string()
                )));
            }
            GuessResult::Incorrect => msg_to_send.push(msg),
            GuessResult::AlreadyGuessed => msg_to_send.push(json!(ChatMessage::new(
                msg["id"].as_i64().unwrap(),
                "Already guessed correctly!".to_string()
            ))),
            GuessResult::Almost => msg_to_send.push(json!(ChatMessage::new(
                msg["id"].as_i64().unwrap(),
                "Close!".to_string()
            ))),
            GuessResult::Drawing => msg_to_send.push(json!(ChatMessage::new(
                msg["id"].as_i64().unwrap(),
                "Drawer may not chat!".to_string()
            ))),
            GuessResult::Spectating => msg_to_send.push(json!(ChatMessage::new(
                msg["id"].as_i64().unwrap(),
                "Spectators may not chat!".to_string()
            ))),
        }
    } else if msg["kind"].eq("disconnect") {
        let id = msg["id"].as_i64().unwrap();
        lobby.remove_player(id);
        if !lobby.game_state().lock().unwrap().in_game {
            clean_up_lobby = true;
        }
        if lobby.all_guessed() {
            // cannot join ifs due to lock acquiring
            clean_up_lobby = true;
        }
        msg_to_send.push(json!(PlayersUpdate::new(
            lobby.players().lock().unwrap().to_vec()
        )));
    } else if msg["kind"].eq("time_up") {
        clean_up_lobby = true;
    } else if msg["kind"].eq("add_line") {
        lobby.add_line_msg(msg.clone());
        msg_to_send.push(msg);
    } else {
        msg_to_send.push(msg);
    }

    if clean_up_lobby {
        //needed to allow timer to start game again
        lobby.cleanup_lobby_after_end_game();
    }

    if send_update {
        msg_to_send.push(json!(GameStateUpdate::new(
            lobby.game_state().lock().unwrap().clone()
        )));
        msg_to_send.push(json!(PlayersUpdate::new(
            lobby.players().lock().unwrap().to_vec()
        )));
    }

    msg_to_send
}

/// Loop listening for waiting on MPSC channel and handle sending broadcast messages for a single lobby.
/// This function will run in a separate thread.
///
/// # Arguments
/// * `lobby` - The lobby which will process any actions and messages received.
/// * `lobby_rx` - The channel to receive broadcast messages regarding the specified lobby from.
///
pub(crate) fn check_send_broadcast_messages(
    lobby: Arc<Mutex<LobbyState>>,
    lobby_rx: mpsc::Receiver<Value>,
) {
    loop {
        if let Ok(msg) = lobby_rx.recv() {
            if !msg["kind"].eq("update") {
                println!("Received message: {:?}", msg);
            }
            let msgs_to_send = handle_message(msg, &mut lobby.lock().unwrap());

            for msg in msgs_to_send.iter() {
                let client_txs = lobby.lock().unwrap().client_tx();
                for (client_id, client_tx) in client_txs.iter() {
                    if client_tx.send(msg.clone()).is_err() {
                        lobby.lock().unwrap().remove_client_tx(*client_id);
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
    if time_elapsed.as_secs() > MIN_TIME_BETWEEN_PINGS {
        match send_message(net_info, &json!({"kind": "ping"})) {
            Ok(_) => Some(true),
            Err(_) => Some(false),
        }
    } else {
        None
    }
}

/// Initializes the client for the first time.
///
/// # Arguments
/// * `net_info` - The network information of the client.
/// * `lobby_tx` - The channel to send messages to the broadcast thread.
///
fn client_initialize(net_info: &mut NetworkInfo, lobby_tx: &mpsc::Sender<Value>) {
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

    let _ = lobby_tx.send(json!({"kind": "user_init", "id": net_info.id , "username": username}));
}

/// The main loop to handle each individual client.
///
/// This function should be run in a separate thread.
/// This function reads in the username and create the
/// shared secret for the client and server to communicate
///
/// # Arguments
/// * `net_info` - The network information of the client.
/// * `lobby_tx` - The channel to send messages to the broadcast thread of a lobby.
/// * `client_tx` - The channel to receive messages from the client.
///
pub(crate) fn handle_client(
    mut net_info: NetworkInfo,
    lobby_tx: mpsc::Sender<Value>,
    client_rx: mpsc::Receiver<Value>,
) {
    client_initialize(&mut net_info, &lobby_tx);
    let mut keepalive = Instant::now();
    let player_id = net_info.id;

    //Start of the client thread's main loop to read messages and send keep-alive pings
    loop {
        if let Ok(msg) = read_tcp_message(&mut net_info) {
            let _ = lobby_tx.send(msg);
            keepalive = Instant::now();
        }

        // Check if rx has messages waiting and if yes, send them to the client
        if let Ok(msg) = client_rx.try_recv() {
            let _ = send_message(&mut net_info, &msg);
        }

        match send_ping_message(&mut net_info, Instant::now().duration_since(keepalive)) {
            Some(false) => {
                let _ = lobby_tx.send(json!(DisconnectMessage::new(player_id)));
                return;
            }
            Some(true) => keepalive = Instant::now(),
            None => {}
        }
    }
}
