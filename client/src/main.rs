mod gamestate;
mod messages;
mod network;

use std::io;

fn main() {
    let mut user_input = String::new();
    println!("Enter your name: ");
    io::stdin()
        .read_line(&mut user_input)
        .expect("error: unable to read user input");

    let res = network::connect_to_server("127.0.0.1", 3000, &user_input);
    match res {
        Ok(mut net_info) => network::get_game_state(&mut net_info),
        Err(e) => panic!("Failed to connect to server {}", e),
    };
}
