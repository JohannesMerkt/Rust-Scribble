use egui::Stroke;
use egui::Color32;
use rust_scribble_common::messages_common::ChatMessage;
use rust_scribble_common::gamestate_common::*;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct ClientState {
    /// the clients stroke settings for drawing
    pub stroke: Stroke,
    /// the lines on the canvas
    pub lines: Vec<Line>,
    /// clients text in the input field of the chat section
    pub chat_message_input: String,
    /// all messages in chat
    pub chat_messages: Vec<ChatMessage>,
    /// the game state
    pub game_state: GameState,

}

impl Default for ClientState {
    fn default() -> Self {
        ClientState { 
            stroke: Stroke::new(10.,Color32::RED), 
            lines: Vec::new(), 
            chat_message_input: String::new(), 
            chat_messages: Vec::new(), 
            game_state: GameState::default(),
        }
    }
}

pub struct ClientStatePlugin;

impl Plugin for ClientStatePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<ClientState>();
    }
}