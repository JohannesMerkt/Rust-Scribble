use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use delegate::delegate;
use edit_distance::edit_distance;
use parking_lot::{Condvar as PLCondvar, Mutex as PLMutex};
use rand::Rng;
use rust_scribble_common::gamestate_common::*;
use rust_scribble_common::messages_common::GameStateUpdate;
use serde_json::{json, Value};

use crate::rewardstrategy::RewardStrategy;

pub(crate) const MIN_NUMBER_PLAYERS: usize = 2;
const GAME_TIME: i64 = 500;
// seconds
const MAX_ALLOWED_EDIT_DISTANCE_FOR_ALMOST: usize = 2;

pub struct LobbyState {
    state: Arc<Mutex<LobbyStateInner>>,
    started_lock: Arc<(PLMutex<bool>, PLCondvar)>,
}

impl LobbyState {
    pub fn default(words: Vec<String>, reward_strategy: &'static dyn RewardStrategy, lobby_tx: mpsc::Sender<Value>) -> Self {
        LobbyState {
            state: Arc::new(Mutex::new(LobbyStateInner::default(words, reward_strategy, lobby_tx))),
            started_lock: Arc::new((PLMutex::new(false), PLCondvar::new())),
        }
    }

    pub fn start_game_on_timer(&mut self, secs: u64) {
        let local_state = self.state.clone();
        let local_started = self.started_lock.clone();
        let tx = self.lobby_tx();

        thread::spawn(move || {
            println!("Init startup thread with {} secs", &secs);
            let (lock, cvar) = &*local_started;
            let mut started = lock.lock();

            // no spurious wakeups in parking_lot cvars
            cvar.wait_for(&mut started, Duration::from_secs(secs));
            if !(*started) && local_state.lock().unwrap().all_ready() {
                local_state.lock().unwrap().start_game();
                Self::start_timer_thread(local_state.clone(), tx.clone());
                *started = true;
                tx.send(json!({"kind": "update_requested"}))
                    .expect("Lobby has lost channel connection to network!");
            } // if already true, another startup thread has started the game already
            cvar.notify_all(); // other startup threads are notified and will terminate as started is already set to true
            println!(
                "Debug: Startup Thread with {} secs terminated (early)",
                secs
            )
        });
    }

    fn start_timer_thread(state_ref: Arc<Mutex<LobbyStateInner>>, lobby_tx: mpsc::Sender<Value>) {
        let tick = schedule_recv::periodic(Duration::from_secs(1));
        thread::spawn(move || {
            loop {
                tick.recv().unwrap();
                let state = state_ref.lock().unwrap();
                let mut game_state = state.game_state.lock().unwrap();
                if !game_state.in_game { break; }
                let new_time = game_state.time - 1;
                game_state.time = new_time;
                // Timer could be implemented clientside to save some network traffic,
                // but as to not cause problems with client side code at this late stage of the
                // project I'll implement this workaround for now
                let _ = lobby_tx.send(json!(GameStateUpdate::new(game_state.clone())));
                drop(game_state);
                drop(state);
                if new_time == 0 {
                    let mut state = state_ref.lock().unwrap();
                    state.end_game();
                    drop(state);
                    lobby_tx.send(json!({"kind": "time_up"}))
                        .expect("Lobby has lost channel connection to network!");
                    break;
                }
            }
        });
    }

    pub fn cleanup_lobby_after_end_game(&mut self) {
        *self.started_lock.0.lock() = false;
    }

    delegate! {
        to self.state.lock().unwrap() {
            pub fn add_player(&mut self, id: i64, name: String);
            pub fn remove_player(&mut self, player_id: i64);
            pub fn set_ready(&mut self, player_id: i64, status: bool);
            pub fn all_ready(&self) -> bool;
            pub fn chat_or_correct_guess(&mut self, player_id: i64, message: &str) -> GuessResult;
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
    pub fn lobby_tx(&self) -> mpsc::Sender<Value> {
        self.state.lock().unwrap().lobby_tx.clone()
    }
    pub fn client_tx(&self) -> BTreeMap<i64, mpsc::Sender<Value>> {
        self.state.lock().unwrap().client_txs.clone()
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
    pub lobby_tx: mpsc::Sender<Value>,
    pub client_txs: BTreeMap<i64, mpsc::Sender<Value>>,
    pub reward_strategy: &'static dyn RewardStrategy,
}


impl LobbyStateInner {
    /// Creates a new ServerStateInner with the given word list and tx.
    ///
    /// # Arguments
    ///   * `words` - The vector word list to use for the game.
    ///   * `lobby_tx` - The tx mpsc to send updates to the clients.
    ///   * `reward_strategy` - The reward strategy to use for the game.
    /// It determines how points are awarded for correct guesses.
    pub fn default(words: Vec<String>, reward_strategy: &'static dyn RewardStrategy, lobby_tx: mpsc::Sender<Value>) -> Self {
        LobbyStateInner {
            game_state: Arc::new(Mutex::new(GameState::default())),
            players: Arc::new(Mutex::new(Vec::new())),
            word_list: Arc::new(Mutex::new(words)),
            lobby_tx,
            client_txs: BTreeMap::new(),
            reward_strategy,
        }
    }

    /// Adds a player' communication channel to the game.
    ///
    /// # Arguments
    ///   * `id` - The id of the player.
    ///   * `tx` - The tx mpsc to send updates to the clients.
    pub fn add_client_tx(&mut self, id: i64, tx: mpsc::Sender<Value>) {
        self.client_txs.insert(id, tx);
    }

    /// Removes a client tx from the game and removes the player id.
    ///
    /// # Arguments
    ///   * `id` - The id of the player.
    ///
    pub fn remove_client_tx(&mut self, id: i64) {
        self.remove_player(id);
        self.client_txs.remove(&id);
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
            if players.len() < MIN_NUMBER_PLAYERS {
                end_game = true;
            }
        }

        if end_game {
            self.end_game();
        }
    }

    /// Check if all players are ready.
    pub fn all_ready(&self) -> bool {
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
    pub fn set_ready(&mut self, player_id: i64, status: bool) {
        //Set player with player_id to ready
        let mut players = self.players.lock().unwrap();
        if let Some(player) = players.iter_mut().find(|player| player.id == player_id) {
            player.ready = status;
        }
    }

    /// Check if all players have guessed the word.
    ///
    /// # Returns
    ///  * `true` - If all players have guessed the word.
    /// * `false` - If not all players have guessed the word.
    pub fn all_guessed(&mut self) -> bool {
        if self.players.lock().unwrap().iter().all(|player| {
            !player.playing
                || !player.drawing && player.guessed_word
                || player.drawing && !player.guessed_word
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
    pub fn chat_or_correct_guess(&mut self, player_id: i64, message: &str) -> GuessResult {
        let game_state = self.game_state.lock().unwrap();
        let mut players = self.players.lock().unwrap();
        let nr_players_finished = Self::calc_position_finished(&*players);
        let total_nr_of_players = players.len();
        for player in &mut players.iter_mut() {
            if game_state.in_game && player.id == player_id {
                if !player.playing {
                    return GuessResult::Spectating;
                } else if player.drawing {
                    return GuessResult::Drawing;
                } else {
                    if player.guessed_word {
                        return GuessResult::AlreadyGuessed;
                    }
                    if game_state.word.to_lowercase().eq(&message.to_lowercase()) {
                        player.guessed_word = true;
                        self.reward_strategy.reward_points_to_player(player, total_nr_of_players, nr_players_finished);
                        return GuessResult::Correct;
                    }
                    if edit_distance(&*game_state.word.to_lowercase(), &message.to_lowercase())
                        <= MAX_ALLOWED_EDIT_DISTANCE_FOR_ALMOST {
                        return GuessResult::Almost;
                    }
                }
            }
        }
        GuessResult::Incorrect
    }

    fn calc_position_finished(all_players: &[Player]) -> usize {
        all_players.iter().filter(|p| p.guessed_word).count()
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
        game_state.time = GAME_TIME;
        let drawer_index = rand::thread_rng().gen_range(0, players.len());
        for (index, player) in (&mut players.iter_mut()).enumerate() {
            if drawer_index == index {
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

pub enum GuessResult {
    Correct,
    Incorrect,
    AlreadyGuessed,
    Almost,
    Drawing,
    Spectating,
}
