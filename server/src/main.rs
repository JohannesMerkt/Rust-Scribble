mod gamestate;
mod network;

use std::sync::Mutex;
fn main() {
    //TODO need to add a broadcast vector to hold broadcast messages from all threads
    let game_state = Mutex::new(gamestate::GameState::new());

    network::tcp_server(game_state);
}
