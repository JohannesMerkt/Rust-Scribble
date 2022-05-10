mod gamestate;
mod network;
fn main() {
    let mut global_state = gamestate::GameState::new();

    network::tcp_server();
}
