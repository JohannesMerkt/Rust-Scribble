use bevy::prelude::*;
use crate::network;
use crate::gamestate;
use serde_json::json;

pub struct NetworkState {
    /// player id that has been given by the server matching with player id in player list
    pub id: Option<i32>,
    /// client player name
    pub name: String,
    /// client input for server address to connect to 
    pub address: String,
    /// client input for server port number to connect to
    pub port: u16,
    // network info if none then not connected
    pub info: Option<network::NetworkInfo>
}

impl Default for NetworkState {
    fn default() -> Self {
        NetworkState {
            id: None,
            name: "Player".to_string(),
            address: "127.0.0.1".to_string(),
            port: 3000,
            info: None
        }
        
    }
}

struct CheckNetworkTimer(Timer);

pub fn connect(mut networkstate: ResMut<NetworkState>) {
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

pub fn send_chat_message(mut networkstate: ResMut<NetworkState>, mut gamestate: ResMut<gamestate::GameState>) {
    let msg = json!({
        "kind": "chat_message",
        "username": networkstate.name,
        "message": gamestate.chat_message_input,
    });
    
    if let Some(network_info) = networkstate.info.as_mut() {
        let _ = network::send_message(network_info, msg);
    }
    gamestate.chat_message_input = "".to_string();
}

fn update_network(time: Res<Time>, mut timer: ResMut<CheckNetworkTimer>, mut networkstate: ResMut<NetworkState>, mut gamestate: ResMut<gamestate::GameState>) {
    if timer.0.tick(time.delta()).just_finished() {
        println!("{}: Check for messages {}", time.seconds_since_startup(), gamestate.chat_messages.len());
        //Read a message from the network
        if let Some(network_info) = networkstate.info.as_mut() {
            if network::message_waiting(network_info) {
                println!("Message waiting");
            }   
            if let Ok(msg)= network::read_messages(network_info, 5) {
                //TODO handle messages 
                for m in msg {
                    println!("{}", m);
                    println!("{}", m["kind"]);

                    if m["kind"].eq("chat_message") {
                        println!("Here");
                        let message = m["message"].as_str().unwrap();
                        println!("Here 2");
                        let chat_message = gamestate::ChatMessage {
                            message: message.to_string(),
                            player_id: 0 // TODO send the player id
                        };
                        gamestate.chat_messages.push(chat_message);
                    }
                    //Display message in the chat window
                    /*if m["kind"].eq("chat_message") {
                        let message = m["message"].as_str().unwrap();
                        let username = m["username"].as_str().unwrap();
                        chat_messages.push(format!("{}: {}", username, message));
                        println!("{} says: {}", username, message);
                    } else if m["kind"].eq("add_line") {
                        let posx:Vec<f64> = m["line"]["posx"].as_array().unwrap().par_iter().map(|pos| pos.as_f64().unwrap()).collect();
                        let posy:Vec<f64> = m["line"]["posy"].as_array().unwrap().par_iter().map(|pos| pos.as_f64().unwrap()).collect();
                        let mut pos_line: Vec<Pos2> = Vec::new();
                        for pos in 0..posx.len() {
                            let pos2 = Pos2{x:posx[pos] as f32, y:posy[pos] as f32};
                            pos_line.push(pos2);
                        }
                        let width = m["line"]["width"].as_f64().unwrap();
                        let color_values: Vec<u8> = m["line"]["color"].as_array().unwrap().par_iter().map(|col| col.as_u64().unwrap() as u8).collect();
                        let color = Color32::from_rgb(color_values[0], color_values[1], color_values[2]);
                        let line: painting::Line = painting::Line {
                            position: pos_line,
                            stroke: egui::Stroke::new(width as f32, color),
                        };
                        painting.all_lines.insert(painting.all_lines.len() - 1, line);
                    }*/
                }
            }
        }
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkState>()
            .insert_resource(CheckNetworkTimer(Timer::from_seconds(1.0, true)))
            .add_system(update_network);
    }
}