use chacha20poly1305::Key;
use serde_json::json;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex, mpsc, RwLock};
use std::time::{Duration, Instant};
use x25519_dalek::PublicKey;
use rayon::prelude::*;

use rust_scribble_common::network_common::*;


use crate::gamestate::GameState;


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
    player_id: i64,
    tx: &mpsc::Sender<serde_json::Value>,
) {
    println!("RCV: {:?}", msg);

    if msg["kind"].eq("chat_message") {
        let  _ = tx.send(json!({
            "kind": "chat_message",
            "player_id": player_id,
            "message": msg["message"].to_string()
        }));
    } else if msg["kind"].eq("ready") {
        let mut game_state = game_state.lock().unwrap();
        let result = game_state.set_ready(player_id, msg["ready"].as_bool().unwrap());
        if result {
            let _ = tx.send(json!({
                "kind": "start",
                "in_game": game_state.in_game,
                "players": &*game_state.players,
                "time": game_state.time,
                "word": game_state.word //TODO only send to drawer
            }));
        } else {
            let _ = tx.send(json!({
                "kind": "update",
                "in_game": game_state.in_game,
                "players": &*game_state.players,
                "time": game_state.time,
            }));
        }
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
    let remove_clients: Arc<Mutex<Vec<i64>>> = Arc::new(Mutex::new(Vec::new()));

    loop {
        if let Ok(msg) = rx.recv() {
            net_infos.write().unwrap().par_iter_mut().for_each(|net_info| {
                match send_message(&mut net_info.write().unwrap(), &msg) {
                    Ok(_) => {}
                    Err(_) => {
                        remove_clients.lock().unwrap().push(net_info.read().unwrap().id);
                    }
                }
            });
        }

        if remove_clients.lock().unwrap().len() > 0 {
            let mut net_infos = net_infos.write().unwrap();
            for player_id in remove_clients.lock().unwrap().iter() {
                let index = net_infos.iter().position(|x| x.read().unwrap().id == *player_id);
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
fn client_disconnected(net_info: &Arc<RwLock<NetworkInfo>>, game_state: &Arc<Mutex<GameState>>, tx: mpsc::Sender<serde_json::Value>) {
    let net_info = net_info.read().unwrap();
    println!("Client {:?} disconnected", net_info.id);
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
                client_disconnected(&net_info, &game_state, tx);
                break;
            },
            Some(true) => keepalive = Instant::now(),
            None => {},
        }
        
    }
}
