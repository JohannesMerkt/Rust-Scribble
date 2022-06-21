#![crate_name = "rust_scribble_server"]
mod gamestate;
mod network;
mod lobby;

use std::{sync::{Mutex, Arc, mpsc, RwLock}, net::{Ipv4Addr, SocketAddrV4, TcpListener}, io::Write, thread};
use chacha20poly1305::Key;
use clap::Parser;

use crate::lobby::LobbyState;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value_t = 3000)]
    port: u16,
}


fn main() {

    let args = Args::parse();

    tcp_server(args.port);
}

/// Runs the listining server for incoming connections.
/// Starts a new thread for each incoming connection
///
/// # Arguments
/// * `game_state` - The game state to be updated.
/// * `port` - The port to listen on.
///
pub fn tcp_server(port: u16) {
    //TODO move this function into main.rs 
    let loopback = Ipv4Addr::new(0, 0, 0, 0);
    let socket = SocketAddrV4::new(loopback, port);
    let listener = TcpListener::bind(socket).unwrap();

    let game_state = Mutex::new(gamestate::GameState::new());
    let global_gs = Arc::new(game_state);
    let global_lobby = Arc::new(Mutex::new(LobbyState::new()));

    println!("Listening on {}", socket);
    let (tx, rx) = mpsc::channel();

    //Spin off a thread to wait for broadcast messages and send them to all clients
    let arc_net_infos = Arc::new(RwLock::new(Vec::new()));

    let net_infos = Arc::clone(&arc_net_infos);
    thread::spawn(move || network::check_send_broadcast_messages(&net_infos, rx));

    loop {
        let (public_key, secret_key) = network::generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        match tcp_stream.write_all(public_key.as_bytes()) {
            Ok(_) => {
                let net_info = RwLock::new(network::NetworkInfo {
                    username: "".to_string(),
                    tcp_stream,
                    key: *Key::from_slice(public_key.as_bytes()),
                    secret_key,
                });

                let arc_net_info = Arc::new(net_info);
                let thread_gs = Arc::clone(&global_gs);
                let thread_lobby = Arc::clone(&global_lobby);
                let thread_net_info = Arc::clone(&arc_net_info);
                let thread_tx = tx.clone();
                arc_net_infos.write().unwrap().push(arc_net_info);

                thread::spawn(move || {
                    network::handle_client(thread_net_info, thread_gs, thread_lobby, thread_tx);
                });
            }
            Err(e) => println!("Error sending public key to {}: {}", addr, e),
        }
    }
}