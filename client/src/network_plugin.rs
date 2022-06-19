use bevy::prelude::*;
use crate::network;

pub struct NetworkState {
    /// player id that has been given by the server matching with player id in player list
    pub id: Option<i32>,
    /// client player name
    pub name: String,
    /// is client connected to a server?
    pub connected: bool,
    /// client input for server address to connect to 
    pub address: String,
    /// client input for server port number to connect to
    pub port: u16,
    pub info: Option<network::NetworkInfo>
}

impl Default for NetworkState {
    fn default() -> Self {
        NetworkState {
            id: None,
            name: "Player".to_string(),
            connected: false,
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
            networkstate.connected = true;
            networkstate.info = Some(info);
        },
        Err(_) => {
            println!("Could not connect to server");
        }
    }
}

fn update_network(time: Res<Time>, mut timer: ResMut<CheckNetworkTimer>, networkstate: ResMut<NetworkState>) {
    if timer.0.tick(time.delta()).just_finished() {
        println!("{}: {}", time.seconds_since_startup(), networkstate.name);
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