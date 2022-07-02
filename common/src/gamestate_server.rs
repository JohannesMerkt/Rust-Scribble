use rand::Rng;
use rayon::prelude::*;

use crate::gamestate_common::*;

impl GameState {

    pub fn add_player(&mut self, id: i64, name: String) {
        self.players.push(Player { id, name, score: 0, ready: false, drawing: false, playing: false, guessed_word: false});
    }

    pub fn remove_player(&mut self, player_id: i64) {
        // leave ingame when player is drawer
        if self.in_game {
            if let Some(player) = self.players.par_iter().find_any(|&player| player.id == player_id) {
                if player.drawing {
                    self.end_game();
                }
            }
           
        }
        self.players.retain(|player| player.id != player_id);
        // leave ingame when only 1 player
        if self.players.len() < 2 {
            self.end_game();
        }
    }

    pub fn set_ready(&mut self, player_id: i64, status: bool) {
        for player in self.players.iter_mut() {
            if player.id == player_id {
                player.ready = status;
            }
        }
        // check if all are ready to start and enough players
        if !self.in_game && self.players.len() > 1 {
            let mut all_ready = true;
            for player in &mut self.players.iter() {
                if !player.ready {
                    all_ready = false;
                }
            }
            if all_ready {
                self.start_game();
            }
        }
    }

    pub fn chat_or_guess(&mut self, player_id: i64, message: &String) -> bool {
        if self.in_game && self.word.eq(message) {
            for player in &mut self.players.iter_mut() {
                if player.id == player_id && !player.drawing {
                    player.guessed_word = true;
                    player.score += 50;
                }
            }
            // check if everyone has guessed the word
            let mut all_guessed = true;
            for player in &mut self.players.iter() {
                if player.playing && !player.drawing && !player.guessed_word {
                    all_guessed = false;
                }
            }
            if all_guessed {
                self.end_game();
            }
            return all_guessed;
        }
        false
    }

    fn get_random_word(&mut self) { 
        // TODO load from a location
        let words = ["Dog".to_string(),"Cat".to_string(),"Tree".to_string(), "House".to_string()].to_vec();
        let mut rng = rand::thread_rng();
        let word_index = rng.gen_range(0, words.len());
        //Todo encrypt for player
        self.word = words[word_index].clone();
        self.word_length = self.word.len() as i64;
    }

    fn start_game(&mut self) {
        self.in_game = true;
        self.get_random_word();
        self.time = 500;
        let drawer_id = rand::thread_rng().gen_range(1, self.players.len()) as i64;
        for player in &mut self.players.iter_mut() {
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
        self.in_game = false;
        self.word = "".to_string();
        self.word_length = 0;
        self.time = 0;
        for player in &mut self.players.iter_mut() {
            player.guessed_word = false;
            player.playing = false;
            player.ready = false;
            player.drawing = false;
        }
    }

}