use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::sync::{Arc, mpsc, Mutex};
use std::thread;
use rust_scribble_common::network_common::{generate_keypair, NetworkInfo};
use chacha20poly1305::Key;
use std::io::Write;
use crate::{handle_client, LobbyState, network};


/// Runs the listening server for incoming connections.
/// Starts a new thread for each incoming connection
///
/// # Arguments
/// * `game_state` - The game state to be updated.
/// * `port` - The port to listen on.
///
pub fn init_tcp_server(port: u16, words: Vec<String>) {
    let loopback = ip_address;
    let socket = SocketAddrV4::new(loopback, port);
    let listener = TcpListener::bind(socket).unwrap();

    println!("Listening on {}", socket);
    let (server_tx, server_rx) = mpsc::channel();
    let mut next_client_id: i64 = 1;

    let lobby = Arc::new(Mutex::new(LobbyState::default(words, server_tx.clone())));
    //Add words to server state
    let broadcast_server_state = Arc::clone(&lobby);

    // Spawn a new thread for handling broadcast messages
    thread::spawn(move || network::check_send_broadcast_messages(broadcast_server_state, server_rx));

    //Main Server loop - accept connections and spawn a new thread for each one
    loop {
        let (public_key, secret_key) = generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        let send_pk = tcp_stream.write_all(public_key.as_bytes());
        let send_id = tcp_stream.write_all(&next_client_id.to_be_bytes());

        if send_pk.is_ok() && send_id.is_ok() {
            let net_info = NetworkInfo {
                id: next_client_id,
                tcp_stream,
                key: *Key::from_slice(public_key.as_bytes()),
                secret_key: Some(secret_key),
            };

            let (client_tx, thread_rx) = mpsc::channel();
            let thread_tx = server_tx.clone();
            lobby
                .lock()
                .unwrap()
                .add_client_tx(next_client_id, client_tx);

            thread::spawn(move || {
                handle_client(net_info, thread_tx, thread_rx);
            });
            next_client_id += 1;
        } else {
            println!("Error sending public key or id to {}", addr);
        }
    }
}


struct ServerState {
    ip_address: Ipv4Addr,
    port: u16,
    words: Vec<String>
}

impl ServerState {
    pub fn init(ip_address: Ipv4Addr, port: u16, words: Vec<String>) -> Self {
        let server = ServerState {
            ip_address,
            port,
            words
        };

        return server
    }

}
