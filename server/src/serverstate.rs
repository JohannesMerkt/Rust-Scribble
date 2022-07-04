use std::sync::{Mutex, Arc, RwLock};
use rand::Rng;

use rust_scribble_common::gamestate_common::*;
use rust_scribble_common::network_common::*;

pub struct ServerState {
    pub game_state: Arc<Mutex<GameState>>,
    pub net_infos: Arc<RwLock<Vec<Arc<RwLock<NetworkInfo>>>>>,
    pub word_list: Arc<Mutex<Vec<String>>>,
}

impl ServerState {
    pub fn default(words: Vec<String>) -> Self {
        ServerState {
            game_state:  Arc::new(Mutex::new(GameState::default())),
            net_infos: Arc::new(RwLock::new(Vec::new())),
            word_list: Arc::new(Mutex::new(words)),
        }
    }

     pub fn add_player(&mut self, id: i64, name: String) {
        self.game_state.lock().unwrap().players.push(Player::new(id, name));
    }

    pub fn remove_player(&mut self, player_id: i64) {
        // leave ingame when player is drawer
        let mut end_game = false;
        {
            let mut game_state = self.game_state.lock().unwrap();
            if game_state.in_game {
                if let Some(player) = game_state.players.iter_mut().find(|player| player.id == player_id) {
                    if player.drawing {
                        end_game = true;
                    }
                }
            }
            game_state.players.retain(|player| player.id != player_id);
            // leave ingame when only 1 player
            if game_state.players.len() < 2 {
                end_game = true;
            }
        }

        if end_game {
            self.end_game();
        }
    }

    pub fn set_ready(&mut self, player_id: i64, status: bool) {
        let mut all_ready = true;
        {
            let mut game_state = self.game_state.lock().unwrap();
            for player in game_state.players.iter_mut() {
                if player.id == player_id {
                    player.ready = status;
                }
            }
            // check if all are ready to start and enough players
            if !game_state.in_game && game_state.players.len() > 1 {
                for player in &mut game_state.players.iter() {
                    all_ready &= player.ready;
                }
            }
        }
        if all_ready {
                self.start_game();
        }
    }

    pub fn chat_or_guess(&mut self, player_id: i64, message: &String) -> bool {
        let mut all_guessed = true;
        {
            let mut game_state = self.game_state.lock().unwrap();
            if game_state.in_game && game_state.word.eq(message) {
                for player in &mut game_state.players.iter_mut() {
                    if player.id == player_id && !player.drawing {
                        player.guessed_word = true;
                        player.score += 50;
                    }
                }
                for player in &mut game_state.players.iter() {
                    if player.playing && !player.drawing {
                        all_guessed &= player.guessed_word;
                    }
                }
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

    fn start_game(&mut self) {
        self.get_random_word();
        let mut game_state = self.game_state.lock().unwrap();
        game_state.in_game = true;
        game_state.time = 500;
        let drawer_id = rand::thread_rng().gen_range(1, game_state.players.len()) as i64;
        for player in &mut game_state.players.iter_mut() {
            if drawer_id == player.id {
                player.drawing = true;
            }
            else {
                player.drawing = false;
            }
            player.guessed_word = false;
            player.playing = true;
            player.ready = false;
           
        }
    }

    fn end_game(&mut self) {
        let mut game_state = self.game_state.lock().unwrap();
        game_state.in_game = false;
        game_state.word = "".to_string();
        game_state.word_length = 0;
        game_state.time = 0;
        for player in &mut game_state.players.iter_mut() {
            player.guessed_word = false;
            player.playing = false;
            player.ready = false;
            player.drawing = false;
        }
    }

}