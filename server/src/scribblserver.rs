use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::sync::{Arc, mpsc, Mutex};
use std::thread;
use rust_scribble_common::network_common::{generate_keypair, NetworkInfo};
use chacha20poly1305::Key;
use std::io::Write;
use std::sync::mpsc::{Receiver, Sender};
use serde_json::Value;
use crate::{handle_client, LobbyState, network};


pub struct ScribblServer {
    socket: SocketAddrV4,
    words: Vec<String>,
    lobbies: Vec<Arc<Mutex<LobbyState>>>,
    //clients_to_lobbies: BTreeMap<i64, mpsc::Sender<Value>>
}

const OPTIMAL_LOBBY_SIZE: usize = 5;

impl ScribblServer {
    /// Initialize the server with the given ip and port.
    /// To start the server, call the run function on the returned ScribblServer.
    ///
    /// # Arguments
    /// * `ip_address` - The ip address of the server.
    /// * `port` - The port to listen on.
    pub fn init(ip_address: Ipv4Addr, port: u16, words: Vec<String>) -> Self {
        let socket = SocketAddrV4::new(ip_address, port);
        ScribblServer {
            socket,
            words,
            lobbies: Vec::new(),
        }
    }

    /// Runs the listening server for incoming connections.
    /// Starts a new thread for each incoming connection.
    /// Loops indefinitely.
    pub fn run(mut self) {
        println!("Listening on {}", self.socket);
        let (server_tx, server_rx): (Sender<Value>, Receiver<Value>) = mpsc::channel();
        let mut next_client_id: i64 = 1;

        // Spawn a new thread for handling broadcast messages
        //thread::spawn(move || network::check_send_broadcast_messages(broadcast_server_state, server_rx));

        let listener = TcpListener::bind(self.socket).unwrap();

        //Main Server loop - accept connections and spawn a new thread for each one
        loop {
            if let Some((client_id, client_tx)) = self.init_client(&listener, server_tx.clone(), &mut next_client_id) {
                self.assign_lobby(client_id, client_tx, server_tx.clone());
            }
        }
    }

    fn assign_lobby(&mut self, client_id: i64, client_tx: Sender<Value>, server_tx: Sender<Value>) {
        let lobby_ref = self.find_lobby(server_tx);
        let mut lobby = lobby_ref.lock().unwrap();
        lobby.add_client_tx(client_id, client_tx);
    }

    fn find_lobby(&mut self, server_tx: Sender<Value>) -> Arc<Mutex<LobbyState>> {
        for lobby_ref in self.lobbies.iter_mut() {
            let lobby = lobby_ref.lock().unwrap();
            if lobby.players().lock().unwrap().len() < OPTIMAL_LOBBY_SIZE {
                return lobby_ref.clone();
            }
        }
        let new_lobby = Arc::new(Mutex::new(LobbyState::default(self.words.to_vec(), server_tx)));
        self.lobbies.push(new_lobby.clone());
        new_lobby
    }

    fn init_client(&self, listener: &TcpListener, server_tx: Sender<Value>, next_client_id: &mut i64) -> Option<(i64, Sender<Value>)> {
        let (public_key, secret_key) = generate_keypair();
        let (mut tcp_stream, addr) = listener.accept().unwrap();
        println!("Connection received! {:?} is Connected.", addr);

        let send_pk = tcp_stream.write_all(public_key.as_bytes());
        let send_id = tcp_stream.write_all(&next_client_id.to_be_bytes());

        return if send_pk.is_ok() && send_id.is_ok() {
            let net_info = NetworkInfo {
                id: *next_client_id,
                tcp_stream,
                key: *Key::from_slice(public_key.as_bytes()),
                secret_key: Some(secret_key),
            };

            let (client_tx, thread_rx) = mpsc::channel();
            thread::spawn(move || {
                handle_client(net_info, server_tx, thread_rx);
            });
            *next_client_id += 1;

            Some((*next_client_id, client_tx))
        } else {
            println!("Error sending public key or id to {}", addr);
            None
        };
    }
}
