use colored::Color;

struct Board {
    //A vector of pixels/color that represent what has been drawn on the board.
    board: Vec<((i32, i32), Color)>,
}

pub struct GameState {
    //Username of the player and score
    users: Vec<(String, i16)>,
    //Whose turn is it?
    turn: String,
    //The current board
    board: Board,
}

impl GameState {
    pub fn new() -> GameState {
        GameState {
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

    //Change the name of the turn to the next player
    pub fn change_player_turn(&mut self) {
        let mut i = 0;
        for (name, _) in self.users.iter() {
            if name == &self.turn {
                if i == self.users.len() - 1 {
                    self.turn = self.users[0].0.clone();
                } else {
                    self.turn = self.users[i + 1].0.clone();
                }
            }
        }
    }

    pub fn add_to_board(&mut self, x: i32, y: i32, color: Color) {
        self.board.board.push(((x, y), color));
    }

    pub fn remove_from_board(&mut self, x: i32, y: i32) {
        self.board.board.retain(|&((x_, y_), _)| x_ != x || y_ != y);
    }
}
