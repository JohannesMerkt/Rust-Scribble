use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use chacha20poly1305::Key;
use rust_scribble_common::network_common::{generate_keypair, NetworkInfo};
use serde_json::Value;

use crate::rewardstrategy::{EqualRewardStrategy, TimeBasedRewardStrategy};
use crate::{handle_client, lobbystate, network, LobbyState};

pub struct ScribblServer {
    socket: SocketAddrV4,
    words: Vec<String>,
    lobbies: Vec<Arc<Mutex<LobbyState>>>,
}

const OPTIMAL_LOBBY_SIZE: usize = 5;
static REWARD_STRATEGY_GUESSER: TimeBasedRewardStrategy = TimeBasedRewardStrategy {
    full_reward: 100,
    initial_time: lobbystate::GAME_TIME,
};
static REWARD_STRATEGY_DRAWER: EqualRewardStrategy = EqualRewardStrategy { full_reward: 100 };

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
        let mut next_client_id: i64 = 1;

        let listener = TcpListener::bind(self.socket).unwrap();

        //Main Server loop - accept connections and spawn a new thread for each one
        loop {
            if let Some(net_info) = self.accept_client(&listener, &mut next_client_id) {
                let (client_tx, client_rx) = mpsc::channel();
                let lobby = self.assign_lobby(net_info.id, client_tx);
                let lobby_tx = lobby.lock().unwrap().lobby_tx();
                thread::spawn(move || handle_client(net_info, lobby_tx, client_rx));
            }
        }
    }

    fn accept_client(
        &self,
        listener: &TcpListener,
        next_client_id: &mut i64,
    ) -> Option<NetworkInfo> {
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
            *next_client_id += 1;
            Some(net_info)
        } else {
            println!("Error sending public key or id to {}", addr);
            None
        };
    }

    fn assign_lobby(&mut self, client_id: i64, client_tx: Sender<Value>) -> Arc<Mutex<LobbyState>> {
        let lobby_ref = self.find_lobby();
        let mut lobby = lobby_ref.lock().unwrap();
        lobby.add_client_tx(client_id, client_tx);
        lobby_ref.clone()
    }

    fn find_lobby(&mut self) -> Arc<Mutex<LobbyState>> {
        for lobby_ref in self.lobbies.iter_mut() {
            let lobby = lobby_ref.lock().unwrap();
            if lobby.players().lock().unwrap().len() < OPTIMAL_LOBBY_SIZE {
                return lobby_ref.clone();
            }
        }

        //if all lobbies are full, create a new one
        self.setup_new_lobby()
    }

    fn setup_new_lobby(&mut self) -> Arc<Mutex<LobbyState>> {
        println!("Setting up new lobby");
        let (lobby_tx, lobby_rx): (Sender<Value>, Receiver<Value>) = mpsc::channel();
        let new_lobby = Arc::new(Mutex::new(LobbyState::default(
            self.words.to_vec(),
            &REWARD_STRATEGY_GUESSER,
            &REWARD_STRATEGY_DRAWER,
            lobby_tx,
        )));
        self.lobbies.push(new_lobby.clone());
        let lobby_ref = new_lobby.clone();
        // Spawn a new thread for handling broadcast messages
        thread::spawn(|| network::check_send_broadcast_messages(lobby_ref, lobby_rx));
        new_lobby
    }
}
