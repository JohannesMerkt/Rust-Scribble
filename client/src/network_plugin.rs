use bevy::prelude::*;
use crate::clientstate::ClientState;
use crate::network;
use rust_scribble_common::gamestate_common::*;
use rust_scribble_common::messages_common::*;
use rust_scribble_common::network_common::*;
use serde_json::json;

pub struct NetworkState {
    /// client player name
    pub name: String,
    /// client input for server address to connect to
    pub address: String,
    /// client input for server port number to connect to
    pub port: u16,
    // network info if none then not connected
    pub info: Option<NetworkInfo>,
}

impl Default for NetworkState {
    fn default() -> Self {
        NetworkState {
            name: "Player".to_string(),
            address: "127.0.0.1".to_string(),
            port: 3000,
            info: None,
        }
    }
}

struct CheckNetworkTimer(Timer);

pub fn connect(networkstate: &mut ResMut<NetworkState>) {
    let res = network::connect_to_server(
        networkstate.address.as_str(),
        networkstate.port,
        networkstate.name.as_str(),
    );
    match res {
        Ok(info) => {
            networkstate.info = Some(info);
        }
        Err(_) => {
            println!("Could not connect to server");
        }
    }
}

pub fn send_chat_message(networkstate: &mut ResMut<NetworkState>, msg: String) {
    if let Some(network_info) = networkstate.info.as_mut() {
        let msg = json!(ChatMessage::new(network_info.id, msg));
        let _ = send_message(network_info, &msg);
    }
}

pub fn send_ready(networkstate: &mut ResMut<NetworkState>, ready_state: bool) {
    if let Some(network_info) = networkstate.info.as_mut() {
        let msg = json!(ReadyMessage::new(network_info.id, ready_state));
        let _ = send_message(network_info, &msg);
    }
}

pub fn send_disconnect(networkstate: &mut ResMut<NetworkState>) {
    if let Some(network_info) = networkstate.info.as_mut() {
        let msg = json!(DisconnectMessage::new(network_info.id));
        let _ = send_message(network_info, &msg);
    }
}

pub fn send_line(networkstate: &mut ResMut<NetworkState>, line: &mut Line) {
    if let Some(network_info) = networkstate.info.as_mut() {
        let msg = json!(PaintingUpdate::new(network_info.id, line.clone()));
        let _ = send_message(network_info, &msg);
    }
}

fn handle_messsages(network_info: &mut NetworkInfo, clientstate: &mut ClientState) {
    if let Ok(msg) = network::read_messages(network_info, 5) {
        for m in msg {
            println!("{}", m);
            println!("{}", m["kind"]);

            if m["kind"].eq("chat_message") {
                let message = m["message"].as_str().unwrap();
                let player_id = m["id"].as_i64().unwrap();
                let chat_message = ChatMessage::new(player_id, message.to_string());
                clientstate.chat_messages.push(chat_message);
            } else if m["kind"].eq("update") {
                if let Ok(new_gs) = serde_json::from_str(&m["game_state"].to_string()) {
                    clientstate.game_state = new_gs;
                }
            } else if m["kind"].eq("player_update") {
                if let Ok(new_gs) = serde_json::from_str(&m["players"].to_string()) {
                    clientstate.players = new_gs;
                }
            } else if m["kind"].eq("add_line") {
                if let Ok(line) = serde_json::from_str(&m["line"].to_string()) {
                    let length = clientstate.lines.len();
                    clientstate.lines.insert(length - 1, line);
                }
            }
        }
    }
}

fn update_network(
    time: Res<Time>,
    mut timer: ResMut<CheckNetworkTimer>,
    mut networkstate: ResMut<NetworkState>,
    mut clientstate: ResMut<ClientState>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        if let Some(mut network_info) = networkstate.info.as_mut() {
            if message_waiting(network_info) {
                handle_messsages(&mut network_info, &mut clientstate)
            }
        }
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkState>()
            .insert_resource(CheckNetworkTimer(Timer::from_seconds(0.25, true)))
            .add_system(update_network);
    }
}
