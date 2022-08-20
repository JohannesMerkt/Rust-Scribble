use crate::gamestate_common::{GameState, Line, Player};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    /// kind of message
    pub kind: String,
    /// id of player who sent the message
    pub id: i64, // TODO use smaller number? u8 ?
    /// the message the player has sent
    pub message: String,
}

impl ChatMessage {
    pub fn new(id: i64, message: String) -> Self {
        ChatMessage {
            kind: "chat_message".to_string(),
            id,
            message,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ReadyMessage {
    pub kind: String,
    pub id: i64,
    pub ready: bool,
}

impl ReadyMessage {
    pub fn new(id: i64, ready: bool) -> Self {
        ReadyMessage {
            kind: "ready".to_string(),
            id,
            ready,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct DisconnectMessage {
    pub kind: String,
    pub id: i64,
}

impl DisconnectMessage {
    pub fn new(id: i64) -> Self {
        DisconnectMessage {
            kind: "disconnect".to_string(),
            id,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct GameStateUpdate {
    pub kind: String,
    pub id: i64,
    pub game_state: Value,
}

impl GameStateUpdate {
    pub fn new(game_state: GameState) -> Self {
        GameStateUpdate {
            kind: "update".to_string(),
            id: 0,
            game_state: json!(game_state),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PlayersUpdate {
    pub kind: String,
    pub id: i64,
    pub players: Value,
}

impl PlayersUpdate {
    pub fn new(players: Vec<Player>) -> Self {
        PlayersUpdate {
            kind: "player_update".to_string(),
            id: 0,
            players: json!(players),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PaintingUpdate {
    pub kind: String,
    pub id: i64,
    pub line: Value,
}

impl PaintingUpdate {
    pub fn new(id: i64, line: Line) -> Self {
        PaintingUpdate {
            kind: "add_line".to_string(),
            id,
            line: json!(line),
        }
    }

    pub fn clear_all(id: i64) -> Self {
        PaintingUpdate {
            kind: "clear_all_lines".to_string(),
            id,
            line: Value::Null,
        }
    }

    pub fn clear_last(id: i64) -> Self {
        PaintingUpdate {
            kind: "clear_last_line".to_string(),
            id,
            line: Value::Null,
        }
    }
}
