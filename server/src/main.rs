#![crate_name = "rust_scribble_server"]
mod network;
mod lobbystate;
mod serverstate;

use crate::network::handle_client;
use crate::lobbystate::LobbyState;
use chacha20poly1305::Key;
use clap::Parser;
use rust_scribble_common::network_common::{generate_keypair, NetworkInfo};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    path::Path,
    sync::{Arc, mpsc, Mutex},
    thread,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = 3000)]
    port: u16,
    #[clap(short, long, value_parser, default_value = "assets/words.txt")]
    words: String,
}

/// Main function for the server.
///
fn main() {
    let args = Args::parse();

    let words = read_words_from_file(args.words);

    serverstate::init_tcp_server(args.port, words);
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
