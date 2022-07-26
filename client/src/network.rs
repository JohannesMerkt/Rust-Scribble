use chacha20poly1305::Key;
use std::error;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::str;
use std::time::Duration;
use x25519_dalek::PublicKey;
use rust_scribble_common::network_common::*;

/// Try and read messages from the server
/// 
/// # Arguments
/// * `net_info` - A mutable reference to the NetworkInfo struct
/// * `number_of_messages` - The number of messages to try and read. 
/// 
/// # Returns
/// * `Ok(messages) - A vector of JSON value messages
/// * `Err(error) - An error if something went wrong
/// 
pub fn read_messages(
    net_info: &mut NetworkInfo,
    n_msg_to_read: u8,
) -> Result<Vec<serde_json::Value>, Box<dyn error::Error>> {
    
    let mut messages = Vec::new();
    for _ in 0..=n_msg_to_read {
        match read_tcp_message(net_info) {
            Ok(msg) => {
                println!("RCV: {}", &msg.to_string());
                messages.push(msg);
            }
            Err(_) => {
                break;
            }
        }
    }

    Ok(messages)
}


/// Connects to the server and returns a NetworkInfo struct
/// 
/// Attempts to connect to the server and generate a 
/// shared key for encrypted communication with the server.
/// 
/// # Arguments
/// * `ip_addr` - The address of the server.
/// * `port` - The port of the server.
/// * `username` - The username of the client.
/// 
/// # Returns
/// * `Ok(net_info)` - A NetworkInfo struct containing the tcp_stream and the key.
/// * `Err(e)` - The error that occurred.
/// 
pub fn connect_to_server(ip_addr: &str, port: u16, username: &str) -> Result<NetworkInfo, Error> {
    let (public_key, secret_key) = generate_keypair();

    let ip_addr = ip_addr.parse::<std::net::Ipv4Addr>().unwrap();
    let socket = std::net::SocketAddrV4::new(ip_addr, port);
    if let Ok(mut tcp_stream) = TcpStream::connect(socket) {
        println!("Connected to the server!");

        let mut buffer = [0; 32];
        let _ = tcp_stream.read(&mut buffer)?;
        let server_key: PublicKey = PublicKey::from(buffer);
        let mut id_buffer = [0; 8];
        let _ = tcp_stream.read(&mut id_buffer)?;
        let id: i64 = i64::from_be_bytes(id_buffer);

        println!("Received id {}!", id);
        tcp_stream.write_all(public_key.as_bytes())?;
        tcp_stream.write_all(username.as_bytes())?;

        let _ = tcp_stream.set_read_timeout(Some(Duration::from_millis(30)));

        let shared_secret = secret_key.diffie_hellman(&server_key);
        let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());
        
        Ok(NetworkInfo {
            id,
            tcp_stream,
            key,
            secret_key: None,
        })
    } else {
        Err(Error::new(ErrorKind::Other, "Failed to connect to server"))
    }
}
