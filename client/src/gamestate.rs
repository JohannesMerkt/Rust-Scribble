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
