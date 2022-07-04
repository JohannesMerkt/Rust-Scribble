#![crate_name = "rust_scribble_server"]
mod network;

use std::{sync::{Arc, mpsc, RwLock}, net::{Ipv4Addr, SocketAddrV4, TcpListener}, io::{Write, BufReader, BufRead}, thread, path::Path, fs::File};
use chacha20poly1305::Key;
use clap::Parser;
use rust_scribble_common::network_common::{generate_keypair, NetworkInfo};
use network::handle_client;

use crate::network::ServerState;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = 3000)]
    port: u16,
    #[clap(short, long, value_parser, default_value = "assets/words.txt")]
    words: String,
}

// Get Words from File and put them in a vector
fn read_words_from_file(filename: impl AsRef<Path>) -> Vec<String> {
    let file = File::open(filename).expect("no such file");
    let buf = BufReader::new(file);
    buf.lines()
        .map(|l| l.expect("Could not parse line"))
        .collect()
}

fn main() {

    let args = Args::parse();

    let words = read_words_from_file(args.words);
    println!("{:?}", words);

    tcp_server(args.port, words);
}

/// Runs the listening server for incoming connections.
/// Starts a new thread for each incoming connection
///
/// # Arguments
/// * `game_state` - The game state to be updated.
/// * `port` - The port to listen on.
///
pub fn tcp_server(port: u16, words: Vec<String>) {
    let loopback = Ipv4Addr::new(0, 0, 0, 0);
    let socket = SocketAddrV4::new(loopback, port);
    let listener = TcpListener::bind(socket).unwrap();

    println!("Listening on {}", socket);
    let (tx, rx) = mpsc::channel();
    let mut next_client_id: i64 = 1;

    let server_state = Arc::new(ServerState::default(words));
    //Add words to server state
    let broadcast_server = Arc::clone(&server_state);

    // Spawn a new for handling broadcast messages
    thread::spawn(move || network::check_send_broadcast_messages(broadcast_server, rx));

    loop {
        let (public_key, secret_key) = generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        //TODO Clean up this nested mess
        match tcp_stream.write_all(public_key.as_bytes()) {
            Ok(_) => {
                match tcp_stream.write_all(&next_client_id.to_be_bytes()) {
                    Ok(_) => {
                        let net_info = RwLock::new(NetworkInfo {
                            id: next_client_id,
                            tcp_stream,
                            key: *Key::from_slice(public_key.as_bytes()),
                            secret_key: Some(secret_key),
                        });

                        let arc_net_info = Arc::new(net_info);
                        let thread_gs = Arc::clone(&server_state.game_state);
                        let thread_net_info = Arc::clone(&arc_net_info);
                        let thread_tx = tx.clone();
                        server_state.net_infos.write().unwrap().push(arc_net_info);

                        thread::spawn(move || {
                            handle_client(thread_net_info, thread_gs, thread_tx);
                        });
                        next_client_id += 1;
                    }
                    Err(_e) => println!("Error sending id"),
                }
            }
            Err(e) => println!("Error sending public key to {}: {}", addr, e),
        }
    }
}