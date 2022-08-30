#![crate_name = "rust_scribble_server"]

use std::{
    fs::File,
    io::{BufRead, BufReader},
    net::Ipv4Addr,
    path::Path,
};

use clap::Parser;

use crate::lobbystate::LobbyState;
use crate::network::handle_client;
use crate::scribblserver::ScribblServer;

mod network;
mod lobbystate;
mod scribblserver;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = 3000)]
    port: u16,
    #[clap(short, long, value_parser, default_value = "assets/words.txt")]
    words: String,
}

/// Main function for setting up and running a scribbl server.
fn main() {
    let args = Args::parse();

    let words = read_words_from_file(args.words);
    let loopback = Ipv4Addr::new(0, 0, 0, 0);
    let server = ScribblServer::init(loopback, args.port, words);
    server.run()
}

/// Get Words from File and put them in a vector
///
/// # Arguments
/// * `filename` - The path to the file containing the words.
///
/// # Returns
/// * Vec<String> - A vector of strings containing the words.
fn read_words_from_file(filename: impl AsRef<Path>) -> Vec<String> {
    let file = File::open(filename).expect("no such file");
    let buf = BufReader::new(file);
    buf.lines()
        .map(|l| l.expect("Could not parse line"))
        .collect()
}
