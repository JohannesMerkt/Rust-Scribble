use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LobbyState {
    //(Username, Ready Status)
    users: Vec<(String, bool)>,
    seconds_til_game_start: Option<u32>,
}

impl LobbyState {
    pub fn new() -> LobbyState {
        LobbyState {
            users: vec![],
            seconds_til_game_start: None,
        }
    }

    pub fn add_player(&mut self, username: String) {
        self.users.push((username, false));
    }

    pub fn remove_player(&mut self, username: String) {
        self.users.retain(|(u, _)| u != &username);
    }

    //Update the ready status of a player
    pub fn set_ready(&mut self, username: String, status: bool) {
        //I'm not sure why this is required? removes quotes
        let username = username[1..username.len()-1].to_string();

        for (u, s) in &mut self.users.iter_mut() {
            if username.eq(u) {
                *s = status;
            }
        }
    }

    pub fn set_seconds_til_game_start(&mut self, seconds: u32) {
        self.seconds_til_game_start = Some(seconds);
    }

}