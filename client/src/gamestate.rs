use egui::Stroke;
use egui::Pos2;
use egui::Color32;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;

#[derive(Serialize, Deserialize)]
struct Line {
    positions: Vec<Pos2>,
    stroke: Stroke
}

#[derive(Serialize, Deserialize)]
struct Player {
    /// player id
    id: i32,
    /// name of the player
    name: String,
    /// the score of the player
    score: i32,
    /// is the player in lobby ready to play
    ready: bool,
    /// is the player drawing or guessing?
    drawing: bool,
    /// is player playing or spectating?
    playing: bool,
    /// has player guessed the word?
    guessed_word: bool,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    /// id of player who sent the message
    player_id: i32,
    /// the message the player has sent
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct GameState {
    /// are we in lobby or ingame?
    lobby: bool,
    /// the clients stroke settings for drawing
    stroke: Stroke,
    /// the lines on the canvas
    lines: Vec<Line>,
    /// clients text in the input field of the chat section
    chat_message_input: String,
    /// all messages in chat
    chat_messages: Vec<ChatMessage>,
    /// all players in the lobby
    players: Vec<Player>,
    /// the word that has to be drawn (only populated when drawing)
    word: String,
    /// remaining time for round in seconds
    time: i32,
}

impl Default for GameState {
    fn default() -> Self {
        GameState { 
            lobby: true, 
            stroke: Stroke::new(10.,Color32::BLACK), 
            lines: Vec::new(), 
            chat_message_input: String::new(), 
            chat_messages: Vec::new(), 
            players: Vec::new(), 
            word: String::new(), 
            time: 0 
        }
    }
}

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<GameState>();
    }
}