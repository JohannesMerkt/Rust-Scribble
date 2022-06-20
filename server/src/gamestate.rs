use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Pos2 {
    /// How far to the right.
    pub x: f32,
    /// How far down.
    pub y: f32,
}

#[derive(Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8
}

#[derive(Serialize, Deserialize)]
pub struct Line {
    pub positions: Vec<Pos2>,
    pub width: f32,
    pub color: Color
}

#[derive(Serialize, Deserialize)]
pub struct Player {
    /// player id
    pub id: i64,
    /// name of the player
    pub name: String,
    /// the score of the player
    pub score: i32,
    /// is the player in lobby ready to play
    pub ready: bool,
    /// is the player drawing or guessing?
    pub drawing: bool,
    /// is player playing or spectating?
    pub playing: bool,
    /// has player guessed the word?
    pub guessed_word: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    /// id of player who sent the message
    pub player_id: i64,
    /// the message the player has sent
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct GameState {
    /// are we in lobby or ingame?
    pub in_game: bool,
    /// the lines on the canvas
    pub lines: Vec<Line>,
    /// all messages in chat
    pub chat_messages: Vec<ChatMessage>,
    /// all players in the lobby
    pub players: Vec<Player>,
    /// the word that has to be drawn (only visible to drawer)
    pub word: String,
    /// remaining time for round in seconds
    pub time: i32,
}

impl GameState {
    pub fn new() -> GameState {
        GameState {
            in_game: false,
            lines: Vec::new(),
            chat_messages: Vec::new(),
            players: Vec::new(),
            word: "".to_string(),
            time: 0
        }
    }

    pub fn add_player(&mut self, id: i64, name: String) {
        self.players.push(Player { id: id, name: name.clone(), score: 0, ready: false, drawing: false, playing: false, guessed_word: false});
    }

    pub fn remove_player(&mut self, player_id: i64) {
        // leave ingame when player is drawer
        if self.in_game {
            let player = self.players.iter().find(|&player| player.id == player_id).unwrap();
            if player.drawing {
                self.end_game();
            }
        }
        self.players.retain(|player| player.id != player_id);
        // leave ingame when only 1 player
        if self.players.len() < 2 {
            self.end_game();
        }
    }

    pub fn set_ready(&mut self, player_id: i64, status: bool) -> bool {
        for player in &mut self.players.iter_mut() {
            if player.id == player_id {
                player.ready = status;
            }
        }
        // check if all are ready to start
        if !self.in_game {
            let mut all_ready = true;
            for player in &mut self.players.iter() {
                if !player.ready {
                    all_ready = false;
                }
            }
            if all_ready {
                self.start_game();
            }
            return all_ready
        }
        return false
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
        } else {
            self.chat_messages.push(ChatMessage { player_id: player_id, message: message.clone() })
        }
        return false;
    }

    pub fn add_line(&mut self, positions: Vec<Pos2>, width: f32, color: Color) {
        self.lines.push(Line {
            positions: positions,
            width: width,
            color: color
        });
    }

    fn start_game(&mut self) {
        self.in_game = true;
        self.word = "Tree".to_string(); // TODO get random word
        self.time = 500;
        let mut drawer = rand::thread_rng().gen_range(0, self.players.len() - 1);
        for player in &mut self.players.iter_mut() {
            player.drawing = false;
            if drawer == 0 {
                player.drawing = true;
            }
            drawer -= 1;
            player.guessed_word = false;
            player.playing = true;
            player.ready = false;
           
        }
    }

    fn end_game(&mut self) {
        self.in_game = false;
        self.word = "".to_string();
        self.time = 0;
        self.lines = Vec::new();
        for player in &mut self.players.iter_mut() {
            player.guessed_word = false;
            player.playing = false;
            player.ready = false;
            player.drawing = false;
        }
    }

}