mod network;

fn main() {
    let res = network::connect_to_server("127.0.0.1", 3000, "Bob");
    match res {
        Ok(mut net_info) => network::get_game_state(&mut net_info),
        Err(e) => panic!("Failed to connect to server {}", e),
    };
}
