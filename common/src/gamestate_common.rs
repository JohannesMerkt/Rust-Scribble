use serde::{Deserialize, Serialize};

use egui::{Pos2, Stroke};

#[derive(Serialize, Deserialize, Clone)]
pub struct Line {
    pub positions: Vec<Pos2>,
    pub stroke: Stroke,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Player {
    /// player id
    pub id: i64,
    /// name of the player
    pub name: String,
    /// the score of the player
    pub score: i64,
    /// is the player in lobby ready to play
    pub ready: bool,
    /// is the player drawing or guessing?
    pub drawing: bool,
    /// is player playing or spectating?
    pub playing: bool,
    /// has player guessed the word?
    pub guessed_word: bool,
}

impl Player {
    pub fn new(id: i64, name: String) -> Self {
        Player {
            id,
            name,
            score: 0,
            ready: false,
            drawing: false,
            playing: false,
            guessed_word: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GameState {
    /// are we in lobby or ingame?
    pub in_game: bool,
    /// the word that has to be drawn (only visible to drawer)
    pub word: String,
    /// The length of the word
    pub word_length: i64,
    /// remaining time for round in seconds
    pub time: i64,
}

impl GameState {
    pub fn default() -> GameState {
        GameState {
            in_game: false,
            word: "".to_string(),
            word_length: 0,
            time: 0,
        }
    }
}
