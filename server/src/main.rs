#![crate_name = "rust_scribble_server"]
mod gamestate;
mod network;
mod lobby;

use std::sync::Mutex;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value_t = 3000)]
    port: u16,
}


fn main() {
    let game_state = Mutex::new(gamestate::GameState::new());
    let args = Args::parse();

    network::tcp_server(game_state, args.port);
}