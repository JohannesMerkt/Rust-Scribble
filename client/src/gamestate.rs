use egui::Stroke;
use egui::Pos2;
use egui::Color32;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use crate::network_plugin;

#[derive(Serialize, Deserialize)]
pub struct Line {
    pub positions: Vec<Pos2>,
    pub stroke: Stroke
}

#[derive(Serialize, Deserialize)]
pub struct Player {
    /// player id
    pub id: i64, // TODO use smaller number? u8 ?
    /// name of the player
    pub name: String,
    /// the score of the player
    pub score: i64, // TODO use smaller number? i32
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
    pub player_id: i64, // TODO use smaller number? u8 ?
    /// the message the player has sent
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct GameState {
    /// are we in lobby or ingame?
    pub in_game: bool,
    /// the clients stroke settings for drawing
    pub stroke: Stroke,
    /// the lines on the canvas
    pub lines: Vec<Line>,
    /// clients text in the input field of the chat section
    pub chat_message_input: String,
    /// all messages in chat
    pub chat_messages: Vec<ChatMessage>,
    /// all players in the lobby
    pub players: Vec<Player>,
    /// the word that has to be drawn (only populated when drawing)
    pub word: String,
    /// remaining time for round in seconds
    pub time: i64, // TODO use smaller number? i32
}

impl Default for GameState {
    fn default() -> Self {
        GameState { 
            in_game: false, 
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