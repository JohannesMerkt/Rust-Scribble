use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Serialize, Deserialize)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Serialize, Deserialize)]
struct Board {
    //A vector of pixels/color that represent what has been drawn on the board.
    board: Vec<(Point, Color)>,
}
#[derive(Serialize, Deserialize)]
pub struct GameState {
    msg_type: String,
    user: String,
    //Username of the player and score
    users: Vec<(String, i32)>,
    //Whose turn is it?
    turn: String,
    //The current board
    board: Board,
}

impl GameState {
    pub fn new() -> GameState {
        GameState {
            msg_type: "gamestate".to_string(),
            user: "server".to_string(),
            users: vec![],
            turn: "".to_string(),
            board: Board { board: vec![] },
        }
    }

    pub fn add_player(&mut self, username: String) {
        self.users.push((username, 0));
    }

    pub fn remove_player(&mut self, username: String) {
        self.users.retain(|&(ref name, _)| name != &username);
    }

    pub fn add_score(&mut self, username: String, points: i32) {
        for (name, score) in &mut self.users {
            if name == &username {
                *score += points;
            }
        }
    }

    pub fn to_string(&self) -> String {
        let mut string = "".to_string();
        for (name, score) in &self.users {
            string += &format!("{}: {}\n", name, score);
        }
        string
    }

    fn reset_player_scores(&mut self) {
        for &mut (_, ref mut score) in self.users.iter_mut() {
            *score = 0;
        }
    }

    fn reset_board(&mut self) {
        self.board.board.clear();
    }

    pub fn change_player_turn(&mut self) {
        let i = 0;
        for (name, _) in self.users.iter() {
            if name == &self.turn {
                if i == self.users.len() - 1 {
                    self.turn = self.users[0].0.clone();
                } else {
                    self.turn = self.users[i + 1].0.clone();
                }
            }
        }
        self.reset_board();
        self.reset_player_scores();
    }

    pub fn add_to_board(&mut self, x: i32, y: i32, color: Color) {
        self.board.board.push((Point { x, y }, color));
    }

    pub fn remove_from_board(&mut self, x: i32, y: i32) {
        self.board
            .board
            .retain(|&(ref point, _)| point.x != x || point.y != y);
    }
}
