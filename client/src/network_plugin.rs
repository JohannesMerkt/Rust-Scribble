use bevy::prelude::*;
use crate::network;
use crate::gamestate;
use serde_json::json;
use rayon::prelude::*;
use rust_scribble_common::network_common::*;
use rust_scribble_common::messages_common::*;

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

pub fn send_chat_message(networkstate: &mut ResMut<NetworkState>, gamestate: &mut ResMut<gamestate::GameState>) {
    let msg = json!({
        "kind": "chat_message",
        "message": gamestate.chat_message_input,
    });
    
    if let Some(network_info) = networkstate.info.as_mut() {
        let _ = send_message(network_info, &msg);
    }
    gamestate.chat_message_input = "".to_string();
}

pub fn send_ready(networkstate: &mut ResMut<NetworkState>, ready_state: bool) {
    if let Some(network_info) = networkstate.info.as_mut() {
        let msg = json!(ReadyMessage::new(network_info.id, ready_state));
        let _ = send_message(network_info, &msg);
    }
}

pub fn send_line(line: &mut gamestate::Line, networkstate: &mut ResMut<NetworkState>) {
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

fn update_network(time: Res<Time>, mut timer: ResMut<CheckNetworkTimer>, mut networkstate: ResMut<NetworkState>, mut gamestate: ResMut<gamestate::GameState>) {
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
                            let chat_message = gamestate::ChatMessage {
                                message: message.to_string(),
                                player_id
                            };
                            gamestate.chat_messages.push(chat_message);
                        } else if m["kind"].eq("update") { 
                            let in_game = m["in_game"].as_bool().unwrap();
                            let raw_players = m["players"].as_array().unwrap();
                            let mut players: Vec<gamestate::Player> = Vec::new();
                            for raw_player in raw_players {
                                players.push(gamestate::Player {
                                    id: raw_player["id"].as_i64().unwrap(),
                                    name: raw_player["name"].as_str().unwrap().to_string(),
                                    score: raw_player["score"].as_i64().unwrap(),
                                    ready: raw_player["ready"].as_bool().unwrap(),
                                    drawing: raw_player["drawing"].as_bool().unwrap(),
                                    playing: raw_player["playing"].as_bool().unwrap(),
                                    guessed_word: raw_player["guessed_word"].as_bool().unwrap()
                                });
                            }
                            let time = m["time"].as_i64().unwrap();
                            gamestate.in_game = in_game;
                            gamestate.time = time;
                            gamestate.players = players;
                            println!("gamestate {}", gamestate.players.len());
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
                            let line: gamestate::Line = gamestate::Line {
                                positions: pos_line,
                                stroke: egui::Stroke::new(width as f32, color),
                            };
                            let length = gamestate.lines.len();
                            gamestate.lines.insert(length - 1, line);
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