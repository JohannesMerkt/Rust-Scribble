use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    /// kind of message
    pub kind: String,
    /// id of player who sent the message
    pub player_id: i64, // TODO use smaller number? u8 ?
    /// the message the player has sent
    pub message: String,
}

impl ChatMessage {
    pub fn new(player_id: i64, message: String) -> Self {
        ChatMessage {
            kind: "chat_message".to_string(),
            player_id,
            message,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ReadyMessage {
    pub kind: String,
    pub player_id: i64,
    pub ready: bool,
}

impl ReadyMessage {
    pub fn new(player_id: i64, ready: bool) -> Self {
        ReadyMessage {
            kind: "ready".to_string(),
            player_id,
            ready,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct DisconnectMessage {
    pub kind: String,
    pub player_id: i64,
}

impl DisconnectMessage {
    pub fn new(player_id: i64) -> Self {
        DisconnectMessage {
            kind: "disconnect".to_string(),
            player_id,
        }
    }
}