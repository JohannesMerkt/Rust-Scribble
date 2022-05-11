mod gamestate;
mod network;

use std::sync::Mutex;
fn main() {
    let mut game_state = Mutex::new(gamestate::GameState::new());

    network::tcp_server(game_state);
}
