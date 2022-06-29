use bevy::prelude::*;
use crate::network;
use crate::clientstate::*;
use serde_json::json;
use rayon::prelude::*;
use rust_scribble_common::network_common::*;
use rust_scribble_common::messages_common::*;
use rust_scribble_common::gamestate_common::*;

pub struct NetworkState {
    /// client player name
    pub name: String,
    /// client input for server address to connect to 
    pub address: String,
    /// client input for server port number to connect to
    pub port: u16,
    // network info if none then not connected
    pub info: Option<NetworkInfo>
}

impl Default for NetworkState {
    fn default() -> Self {
        NetworkState {
            name: "Player".to_string(),
            address: "127.0.0.1".to_string(),
            port: 3000,
            info: None
        }
        
    }
}

struct CheckNetworkTimer(Timer);

pub fn connect(networkstate: &mut ResMut<NetworkState>) {
    let res = network::connect_to_server(networkstate.address.as_str(), networkstate.port, networkstate.name.as_str());
    match res {
        Ok(info) => {
            networkstate.info = Some(info);
        },
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

pub fn send_line(line: &mut Line, networkstate: &mut ResMut<NetworkState>) {
    let x_positions: Vec<f32> = line.positions.par_iter().map(|pos2| pos2.x).collect();
    let y_positions: Vec<f32> = line.positions.par_iter().map(|pos2| pos2.y).collect();
    let width = line.stroke.width;
    let color = line.stroke.color;
    let msg = json!({
        "kind": "add_line",
        "line": {
            "x_positions": x_positions,
            "y_positions": y_positions,
            "width": width,
            "color": color,
        }
    });
    if let Some(network_info) = networkstate.info.as_mut() {
        let _ = send_message(network_info, &msg);
    }
}

fn update_network(time: Res<Time>, mut timer: ResMut<CheckNetworkTimer>, mut networkstate: ResMut<NetworkState>, mut clientstate: ResMut<ClientState>) {
    if timer.0.tick(time.delta()).just_finished() {

        //Read a message from the network
        //TODO Replace nested IF's
        if let Some(network_info) = networkstate.info.as_mut() {
            if message_waiting(network_info) {
                if let Ok(msg)= network::read_messages(network_info, 5) {
                    //TODO handle messages 
                    for m in msg {
                        println!("{}", m);
                        println!("{}", m["kind"]);

                        if m["kind"].eq("chat_message") {
                            let message = m["message"].as_str().unwrap();
                            let player_id = m["player_id"].as_i64().unwrap();
                            let chat_message = ChatMessage {
                                kind: "chat_message".to_string(),
                                message: message.to_string(),
                                player_id
                            };
                            clientstate.chat_messages.push(chat_message);
                        } else if m["kind"].eq("update") { 
                            if let Ok(new_gs) = serde_json::from_str(&m["game_state"].to_string()) {
                                clientstate.game_state = new_gs;
                            }
                        } else if m["kind"].eq("add_line") {
                            let x_positions:Vec<f64> = m["line"]["x_positions"].as_array().unwrap().iter().map(|pos| pos.as_f64().unwrap()).collect();
                            let y_positions:Vec<f64> = m["line"]["y_positions"].as_array().unwrap().iter().map(|pos| pos.as_f64().unwrap()).collect();
                            let mut pos_line: Vec<egui::Pos2> = Vec::new();
                            for pos in 0..x_positions.len() {
                                let pos2 = egui::Pos2{x:x_positions[pos] as f32, y:y_positions[pos] as f32};
                                pos_line.push(pos2);
                            }
                            let width = m["line"]["width"].as_f64().unwrap();
                            let color_values: Vec<u8> = m["line"]["color"].as_array().unwrap().iter().map(|col| col.as_u64().unwrap() as u8).collect();
                            let color = egui::Color32::from_rgb(color_values[0], color_values[1], color_values[2]);
                            let line: Line = Line {
                                positions: pos_line,
                                stroke: egui::Stroke::new(width as f32, color),
                            };
                            let length = clientstate.lines.len();
                            clientstate.lines.insert(length - 1, line);
                        }
                    }
                }
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