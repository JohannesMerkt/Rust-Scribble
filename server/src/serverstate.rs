use std::sync::mpsc;
use std::sync::{Mutex, Arc, RwLock};
use parking_lot::{Mutex as PLMutex, Condvar as PLCondvar};
use rust_scribble_common::messages_common::{GameStateUpdate, PlayersUpdate};
use serde_json::json;
use std::thread;
use std::time::Duration;
use rand::Rng;
use delegate::delegate;

use rust_scribble_common::gamestate_common::*;
use rust_scribble_common::network_common::*;

pub struct ServerState {
    state: Arc<Mutex<ServerStateInner>>,
    started_lock: Arc<(PLMutex<bool>, PLCondvar)>,
}

// by default only start if all players are ready
const DEFAULT_STARTUP_TIME: u64 = u64::MAX;

impl ServerState {
    pub fn default(words: Vec<String>, tx: mpsc::Sender<serde_json::Value>) -> Self {
        let mut server_state = ServerState {
            state: Arc::new(Mutex::new(ServerStateInner::default(words, tx))),
            started_lock: Arc::new((PLMutex::new(false), PLCondvar::new())),
        };
        server_state.start_game_on_timer(DEFAULT_STARTUP_TIME);
        server_state
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
            if !(*started) {
                local_state.lock().unwrap().start_game();
                *started = true;
                let _ = tx.send(json!(GameStateUpdate::new(0, local_state.lock().unwrap().game_state.lock().unwrap().clone()))).unwrap();
                let _ = tx.send(json!(PlayersUpdate::new(local_state.lock().unwrap().players.lock().unwrap().to_vec())));
            } // if already true, another startup thread has started the game already
            cvar.notify_all(); // other startup threads are notified and will terminate as started is already set to true
            println!("Debug: Startup Thread with {} secs terminated (early)", secs)
        });
    }

    delegate! {
        to self.state.lock().unwrap() {
            pub fn add_player(&mut self, id: i64, name: String);
            pub fn remove_player(&mut self, player_id: i64);
            pub fn set_ready(&mut self, player_id: i64, status: bool) -> bool;
            pub fn chat_or_guess(&mut self, player_id: i64, message: &String) -> bool;
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
    pub fn net_infos(&self) -> Arc<RwLock<Vec<Arc<RwLock<NetworkInfo>>>>> {
        self.state.lock().unwrap().net_infos.clone()
    }
    pub fn word_list(&self) -> Arc<Mutex<Vec<String>>> {
        self.state.lock().unwrap().word_list.clone()
    }
    pub fn tx(&self) -> mpsc::Sender<serde_json::Value> {
        self.state.lock().unwrap().tx.clone()
    }
}


struct ServerStateInner {
    pub game_state: Arc<Mutex<GameState>>,
    pub players: Arc<Mutex<Vec<Player>>>,
    pub net_infos: Arc<RwLock<Vec<Arc<RwLock<NetworkInfo>>>>>,
    pub word_list: Arc<Mutex<Vec<String>>>,
    pub tx: mpsc::Sender<serde_json::Value>,
}

impl ServerStateInner {
    pub fn default(words: Vec<String>, tx: mpsc::Sender<serde_json::Value>) -> Self {
        ServerStateInner {
            game_state: Arc::new(Mutex::new(GameState::default())),
            players: Arc::new(Mutex::new(Vec::new())),
            net_infos: Arc::new(RwLock::new(Vec::new())),
            word_list: Arc::new(Mutex::new(words)),
            tx,
        }
    }

    pub fn add_player(&mut self, id: i64, name: String) {
        self.players.lock().unwrap().push(Player::new(id, name));
    }

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

    pub fn set_ready(&mut self, player_id: i64, status: bool) -> bool {
        let game_state = self.game_state.lock().unwrap();
        let mut players = self.players.lock().unwrap();
        for player in players.iter_mut() {
            if player.id == player_id {
                player.ready = status;
            }
        }
        // check if all are ready to start and enough players
        if !game_state.in_game && players.len() > 1 {
            players.iter().all(|player| player.ready)
        } else {
            false
        }
    }

    pub fn chat_or_guess(&mut self, player_id: i64, message: &String) -> bool {
        let mut all_guessed = true;
        {
            let game_state = self.game_state.lock().unwrap();
            let mut players = self.players.lock().unwrap();
            if game_state.in_game && game_state.word.eq(message) {
                for player in &mut players.iter_mut() {
                    if player.id == player_id && !player.drawing {
                        player.guessed_word = true;
                        player.score += 50;
                    }
                }
                for player in &mut players.iter() {
                    if player.playing && !player.drawing {
                        all_guessed &= player.guessed_word;
                    }
                }
            } else {
                all_guessed = false;
            }
        }
        if all_guessed {
            self.end_game();
        }

        all_guessed
    }

    fn get_random_word(&mut self) {
        let mut game_state = self.game_state.lock().unwrap();
        let mut words = self.word_list.lock().unwrap();
        let word_index = rand::thread_rng().gen_range(0, words.len());
        //Todo encrypt for player
        game_state.word = words[word_index].clone();
        game_state.word_length = words[word_index].len() as i64;
        words.remove(word_index);
    }

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
