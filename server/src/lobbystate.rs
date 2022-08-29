use delegate::delegate;
use parking_lot::{Condvar as PLCondvar, Mutex as PLMutex};
use rand::Rng;
use rust_scribble_common::messages_common::{GameStateUpdate, PlayersUpdate};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rust_scribble_common::gamestate_common::*;

pub struct LobbyState {
    state: Arc<Mutex<LobbyStateInner>>,
    started_lock: Arc<(PLMutex<bool>, PLCondvar)>,
}

impl LobbyState {
    pub fn default(words: Vec<String>, server_tx: mpsc::Sender<serde_json::Value>) -> Self {
        LobbyState {
            state: Arc::new(Mutex::new(LobbyStateInner::default(words, server_tx))),
            started_lock: Arc::new((PLMutex::new(false), PLCondvar::new())),
        }
    }

    pub fn start_game_on_timer(&mut self, secs: u64) {
        let local_state = self.state.clone();
        let local_started = self.started_lock.clone();
        let tx = self.tx();

        thread::spawn(move || {
            println!("Init startup thread with {} secs", &secs);
            let (lock, cvar) = &*local_started;
            let mut started = lock.lock();

            // no spurious wakeups in parking_lot cvars
            cvar.wait_for(&mut started, Duration::from_secs(secs));
            if !(*started) && local_state.lock().unwrap().all_ready() {
                local_state.lock().unwrap().start_game();
                *started = true;
                let _ = tx.send(json!(GameStateUpdate::new(
                    local_state
                        .lock()
                        .unwrap()
                        .game_state
                        .lock()
                        .unwrap()
                        .clone()
                )));
                let _ = tx.send(json!(PlayersUpdate::new(
                    local_state.lock().unwrap().players.lock().unwrap().to_vec()
                )));
            } // if already true, another startup thread has started the game already
            cvar.notify_all(); // other startup threads are notified and will terminate as started is already set to true
            println!(
                "Debug: Startup Thread with {} secs terminated (early)",
                secs
            )
        });
    }

    pub fn end_game(&mut self) {
        *self.started_lock.0.lock() = false;
    }

    delegate! {
        to self.state.lock().unwrap() {
            pub fn add_player(&mut self, id: i64, name: String);
            pub fn remove_player(&mut self, player_id: i64);
            pub fn set_ready(&mut self, player_id: i64, status: bool) -> bool;
            pub fn chat_or_correct_guess(&mut self, player_id: i64, message: &str) -> bool;
            pub fn all_guessed(&mut self) -> bool;
            pub fn add_client_tx(&mut self, id: i64, tx: mpsc::Sender<Value>);
            pub fn remove_client_tx(&mut self, id: i64);
            // start_game should not be accessible directly to keep the interface clean.
            // A countdown of 0 seconds can be used to start immediately
            // but the game is usually started with some small countdown instead
        }
    }

    // delegation for field access has to be implemented manually
    pub fn game_state(&self) -> Arc<Mutex<GameState>> {
        self.state.lock().unwrap().game_state.clone()
    }
    pub fn players(&self) -> Arc<Mutex<Vec<Player>>> {
        self.state.lock().unwrap().players.clone()
    }
    pub fn _word_list(&self) -> Arc<Mutex<Vec<String>>> {
        self.state.lock().unwrap().word_list.clone()
    }
    pub fn tx(&self) -> mpsc::Sender<serde_json::Value> {
        self.state.lock().unwrap().server_tx.clone()
    }
    pub fn client_tx(&self) -> BTreeMap<i64, mpsc::Sender<Value>> {
        self.state.lock().unwrap().client_tx.clone()
    }
}

/// The internal state of a lobby on the server.
///
/// This is the state that is shared between the server and the game logic.
/// The game logic is responsible for updating the state and the server is responsible for sending updates to the clients.
///
struct LobbyStateInner {
    pub game_state: Arc<Mutex<GameState>>,
    pub players: Arc<Mutex<Vec<Player>>>,
    pub word_list: Arc<Mutex<Vec<String>>>,
    pub server_tx: mpsc::Sender<serde_json::Value>,
    pub client_tx: BTreeMap<i64, mpsc::Sender<Value>>,
}

impl LobbyStateInner {
    /// Creates a new ServerStateInner with the given word list and tx.
    ///
    /// # Arguments
    ///     * `words` - The vector word list to use for the game.
    ///    * `tx` - The tx mpsc to send updates to the clients.
    pub fn default(words: Vec<String>, server_tx: mpsc::Sender<serde_json::Value>) -> Self {
        LobbyStateInner {
            game_state: Arc::new(Mutex::new(GameState::default())),
            players: Arc::new(Mutex::new(Vec::new())),
            word_list: Arc::new(Mutex::new(words)),
            server_tx,
            client_tx: BTreeMap::new(),
        }
    }

    /// Adds a player to the game.
    ///
    /// # Arguments
    ///    * `id` - The id of the player.
    ///   * `tx` - The tx mpsc to send updates to the clients.
    pub fn add_client_tx(&mut self, id: i64, tx: mpsc::Sender<Value>) {
        self.client_tx.insert(id, tx);
    }

    /// Removes a client tx from the game and removes the player id.
    ///
    /// # Arguments
    ///   * `id` - The id of the player.
    ///
    pub fn remove_client_tx(&mut self, id: i64) {
        self.remove_player(id);
        self.client_tx.remove(&id);
    }

    /// Adds a player to the game.
    ///
    /// # Arguments
    ///   * `id` - The id of the player.
    ///   * `name` - The name of the player.
    ///
    pub fn add_player(&mut self, id: i64, name: String) {
        self.players.lock().unwrap().push(Player::new(id, name));
    }

    /// Removes a player from the game.
    ///
    /// # Arguments
    ///  * `id` - The id of the player.
    ///
    pub fn remove_player(&mut self, player_id: i64) {
        // leave ingame when player is drawer
        let mut end_game = false;
        {
            let game_state = self.game_state.lock().unwrap();
            let mut players = self.players.lock().unwrap();
            if game_state.in_game {
                if let Some(player) = players.iter_mut().find(|player| player.id == player_id) {
                    if player.drawing {
                        end_game = true;
                    }
                }
            }
            players.retain(|player| player.id != player_id);
            // leave ingame when only 1 player
            if players.len() < 2 {
                end_game = true;
            }
        }

        if end_game {
            self.end_game();
        }
    }

    /// Check if all players are ready.
    fn all_ready(&self) -> bool {
        self.players
            .lock()
            .unwrap()
            .iter()
            .all(|player| player.ready)
    }

    /// Set the ready status of a player.
    ///
    /// # Arguments
    ///   * `player_id` - The id of the player.
    ///  * `status` - The new ready status.
    ///
    pub fn set_ready(&mut self, player_id: i64, status: bool) -> bool {
        //Set player with player_id to ready
        let mut players = self.players.lock().unwrap();
        if let Some(player) = players.iter_mut().find(|player| player.id == player_id) {
            player.ready = status;
            return true;
        }
        //Check if all players are ready
        self.all_ready()
    }

    /// Check if all players have guessed the word.
    ///
    /// # Returns
    ///  * `true` - If all players have guessed the word.
    /// * `false` - If not all players have guessed the word.
    pub fn all_guessed(&mut self) -> bool {
        if self.players.lock().unwrap().iter().all(|player| {
            !player.drawing && player.guessed_word || player.drawing && !player.guessed_word
        }) {
            self.end_game();
            return true;
        }
        false
    }

    /// Check if the message received from the client is a valid guess or chat message.
    ///
    /// # Arguments
    ///  * `player_id` - The id of the player.
    ///  * `message` - The message received from the client.
    ///
    pub fn chat_or_correct_guess(&mut self, player_id: i64, message: &str) -> bool {
        let game_state = self.game_state.lock().unwrap();
        let mut players = self.players.lock().unwrap();
        if game_state.in_game && game_state.word.to_lowercase().eq(&message.to_lowercase()) {
            for player in &mut players.iter_mut() {
                if player.id == player_id && !player.drawing {
                    player.guessed_word = true;
                    player.score += 50;
                    return true;
                }
            }
        }
        false
    }

    /// Gets a new random word from the word list and removes it from the word list.
    ///
    /// # Returns
    /// * `word` - The new random word.
    fn get_random_word(&mut self) {
        let mut game_state = self.game_state.lock().unwrap();
        let mut words = self.word_list.lock().unwrap();
        let word_index = rand::thread_rng().gen_range(0, words.len());
        game_state.word = words[word_index].clone();
        game_state.word_length = words[word_index].len() as i64;
        words.remove(word_index);
    }

    /// Starts a new game.
    pub fn start_game(&mut self) {
        println!("Starting Game");
        self.get_random_word();
        let mut game_state = self.game_state.lock().unwrap();
        let mut players = self.players.lock().unwrap();
        game_state.in_game = true;
        game_state.time = 500;
        let drawer_id = rand::thread_rng().gen_range(1, players.len() + 1) as i64;
        for player in &mut players.iter_mut() {
            if drawer_id == player.id {
                player.drawing = true;
            } else {
                player.drawing = false;
            }
            player.guessed_word = false;
            player.playing = true;
            player.ready = false;
        }
    }

    /// Ends the game.
    fn end_game(&mut self) {
        let mut game_state = self.game_state.lock().unwrap();
        let mut players = self.players.lock().unwrap();
        game_state.in_game = false;
        game_state.word = "".to_string();
        game_state.word_length = 0;
        game_state.time = 0;
        for player in &mut players.iter_mut() {
            player.guessed_word = false;
            player.playing = false;
            player.ready = false;
            player.drawing = false;
        }
    }
}
