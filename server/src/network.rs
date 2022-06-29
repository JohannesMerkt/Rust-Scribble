use chacha20poly1305::Key;
use rust_scribble_common::messages_common::{DisconnectMessage, GameStateUpdate};
use serde_json::json;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex, mpsc, RwLock};
use std::time::{Duration, Instant};
use x25519_dalek::PublicKey;
use rayon::prelude::*;
use rust_scribble_common::network_common::*;
use rust_scribble_common::gamestate_common::*;
use rust_scribble_common::gamestate_server::*;

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
    player_id: i64,
    game_state: &Arc<Mutex<GameState>>,
    tx: &mpsc::Sender<serde_json::Value>,
) {
    println!("RCV: {:?}", msg);

    //TODO create message structs and remove unpack and repacking
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
            let _ = tx.send(json!(GameStateUpdate::new(game_state.clone())));
        }
    } else if msg["kind"].eq("add_line") {
        let _ = tx.send(msg);
    } else if msg["kind"].eq("disconnect") {
        client_disconnected(player_id, game_state);
        let game_state = game_state.lock().unwrap();
        let _ = tx.send(json!(msg));
        let _ = tx.send(json!(GameStateUpdate::new(game_state.clone())));
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

            if msg["kind"].eq("disconnect") {
                let mut remove_clients = remove_clients.lock().unwrap();
                remove_clients.push(msg["player_id"].as_i64().unwrap());
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

            net_infos.write().unwrap().par_iter_mut().for_each(|net_info| {
                match send_message(&mut net_info.write().unwrap(), &msg) {
                    Ok(_) => {}
                    Err(_) => {
                        remove_clients.lock().unwrap().push(net_info.read().unwrap().id);
                    }
                }
            });
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
fn client_disconnected(player_id: i64, game_state: &Arc<Mutex<GameState>>) {
    println!("Client {:?} disconnected", player_id);
    let mut game_state = game_state.lock().unwrap();
    game_state.remove_player(player_id);
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
    game_state: &Arc<Mutex<GameState>>,
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

    let username = username.trim().to_string();

    let client_public: PublicKey = PublicKey::from(buffer);
    let shared_secret = net_info.secret_key.as_ref().unwrap().diffie_hellman(&client_public);
    net_info.key = *Key::from_slice(shared_secret.as_bytes());

    {
        let mut game_state = game_state.lock().unwrap();
        game_state.add_player(net_info.id, username);
        // broadcast all players in lobby when players join
        let _ = tx.send(json!(GameStateUpdate::new(game_state.clone())));
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
/// * `tx` - The channel to send messages to the broadcast thread.
/// 
pub(crate) fn handle_client(
    net_info: Arc<RwLock<NetworkInfo>>,
    game_state: Arc<Mutex<GameState>>,
    tx: mpsc::Sender<serde_json::Value>,
) {
    //TODO handle false case for failure to connect
    let _ = client_initialize(&net_info, &game_state, &tx);
    let mut keepalive = Instant::now();
    let player_id = net_info.read().unwrap().id;

    //Start of the main loop to read messages and send keepalive pings
    //TODO ideally this would be done async or something cleaner
    loop {
        if let Ok(msg) = read_tcp_message(&mut net_info.write().unwrap()) {
            handle_message(msg, player_id, &game_state, &tx);
            keepalive = Instant::now();
        }

        match send_ping_message(&net_info, Instant::now().duration_since(keepalive)) {
            Some(false) => {
                client_disconnected(player_id, &game_state);
                let gs = game_state.lock().unwrap();
                //TODO Remove from NetInfos and send message to all clients
                let _ = tx.send(json!(DisconnectMessage::new(player_id)));
                let _ = tx.send(json!({
                    "kind": "update",
                    "in_game": gs.in_game,
                    "players": &*gs.players,
                    "time": gs.time,
                }));
                break;
            },
            Some(true) => keepalive = Instant::now(),
            None => {},
        }
        
    }
}
