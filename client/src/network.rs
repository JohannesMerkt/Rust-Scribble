use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use serde_json::Value;
use std::error;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::str;
use std::time::Duration;
use x25519_dalek::{EphemeralSecret, PublicKey};


/// Contains all the information about a client connection.
pub struct NetworkInfo {
    /// The id of the client.
    pub id: i64,
    /// The tcp_stream of the client.
    tcp_stream: TcpStream,
    /// The shared secret of the client and server.
    key: Key,
}

/// Generates a new Public Private keypair.
/// 
/// # Returns
/// * `public_key` - A public key.
/// * `secret_key` - A secret key.
/// 
fn generate_keypair() -> (PublicKey, EphemeralSecret) {
    let secret = EphemeralSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

/// Encrypts a JSON message
/// 
/// # Arguments
/// * `json_message` - The message to be encrypted.
/// * `share_key` - The shared key to be used for encryption.
/// 
/// # Returns
/// * `(msg_size, nonce,  ciphermsg, checksum)` - A tuple with the size of the whole message(inclusive nonce, checksum, and message), nonce, the encrypted message and the checksum.
///
fn encrypt_json(json_message: Vec<u8>, shared_key: Key) -> (usize, Nonce, Vec<u8>, u32) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let cipher = ChaCha20Poly1305::new(&shared_key);

    let ciphertext = cipher
        .encrypt(&nonce, &json_message[..])
        .expect("encryption failure!");

    let checksum = crc32fast::hash(&ciphertext);

    //Add 12 bytes for the nonce and 4 bytes for the checksum
    let msg_size = ciphertext.len() + 16;

    (msg_size, nonce, ciphertext, checksum)
}

/// Verifies if the checksum of the chipher text is correct.
/// 
/// # Arguments
/// * `cipher_text` - The cipher text to be verified.
/// * `checksum` - The checksum to be verified.
/// 
fn check_checksum(ciphertext: &[u8], checksum: u32) -> bool {
    let checksum_calc = crc32fast::hash(ciphertext);
    checksum == checksum_calc
}

/// Sends a message to a client.
/// 
/// # Arguments
/// * `tcp_stream` - The tcp_stream of the client.
/// * `net_msg: (usize, Nonce, Vec<u8>, u32)` - The prepared encrypted tuple from encrypt_json() to be sent to the client
/// 
/// # Returns
/// * `Ok(())` - The message was sent successfully.
/// * `Err(e)` - The error that occured.
/// 
fn send_tcp_message(
    tcp_stream: &mut TcpStream,
    net_msg: (usize, Nonce, Vec<u8>, u32),
) -> Result<(), Error> {
    //TODO send 1 message not 3
    tcp_stream.write_all(&usize::to_le_bytes(net_msg.0))?;
    tcp_stream.write_all(&net_msg.1)?;
    tcp_stream.write_all(&net_msg.2)?;
    tcp_stream.write_all(&u32::to_le_bytes(net_msg.3))?;
    Ok(())
}

/// Sends a JSON message to the server
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// * `msg` - The message JSON Value to be sent.
/// 
/// # Returns
/// * `Ok(())` - The message was sent successfully.
/// * `Err(e)` - The error that occured.
/// 
pub fn send_message(net_info: &mut NetworkInfo, msg: Value) -> Result<(), Error> {
    send_tcp_message(
        &mut net_info.tcp_stream,
        encrypt_json(msg.to_string().into_bytes(), net_info.key),
    )
}

/// Checks if any messages are waiting to be read from the network
/// 
/// # Arguments
/// * `net_info` - The network information
/// 
/// # Returns
/// * `true` - If there are messages waiting to be read.
/// 
/// This function should be used in a thread to force updates as soon as a message is waiting to be read.
/// 
pub fn message_waiting(net_info: &mut NetworkInfo) -> bool { 
    let buf = &mut [0; 1];
    let res = net_info.tcp_stream.peek(buf); 
    return res.is_ok() && res.unwrap() > 0; 
}

/// Try and read a message from the server
/// 
/// # Arguments
/// * `net_info` - A mutable reference to the NetworkInfo struct
/// 
/// # Returns
/// * `Ok(messages) - A vector of JSON value messages
/// * `Err(error) - An error if something went wrong
/// 
pub fn read_message(
     net_info: &mut NetworkInfo,
) -> Result<Vec<serde_json::Value>, Box<dyn error::Error>> {
    read_messages(net_info, 1)
}

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
                messages.push(msg);
            }
            Err(_) => {
                break;
            }
        }
    }

    Ok(messages)
}

/// Reads a tcp_message from the server
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// 
/// # Returns
/// * `Ok(msg)` - The message read from the client in JSON format.
/// * `Err(e)` - The error that occured.
/// 
fn read_tcp_message(
    net_info: &mut NetworkInfo,
) -> Result<serde_json::Value, Box<dyn error::Error>> {
    let mut size = [0; (usize::BITS / 8) as usize];
    net_info.tcp_stream.read_exact(&mut size)?;
    let msg_size: usize = usize::from_le_bytes(size);
    
    let mut msg_buf = vec![0; msg_size];
    net_info.tcp_stream.read_exact(&mut msg_buf)?;

    let cipher = ChaCha20Poly1305::new(&net_info.key);
    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);

    let ciphertext = &msg_buf[12..msg_size - 4];
    let checksum: u32 = u32::from_le_bytes(msg_buf[msg_size - 4..msg_size].try_into()?);

    //if check_checksum of ciphertext returns false, throw error
    if !check_checksum(ciphertext, checksum) {
        return Err(Box::new(Error::new(
            ErrorKind::InvalidData,
            "Checksum failed",
        )));
    }

    let json_message = match cipher.decrypt(&nonce, ciphertext) {
        Ok(plaintext) => {
            serde_json::from_slice(&plaintext)?
        }
        Err(_) => {
            println!("Decryption failed!");
            return Err(Box::new(Error::new(ErrorKind::Other, "Decryption failed!")));
        }
    };

    Ok(json_message)
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
/// * `Err(e)` - The error that occured.
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
        println!("Recieved id {}!", id);
        tcp_stream.write_all(public_key.as_bytes())?;
        tcp_stream.write_all(username.as_bytes())?;

        let _ = tcp_stream.set_read_timeout(Some(Duration::from_millis(30)));

        let shared_secret = secret_key.diffie_hellman(&server_key);
        let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());
        
        Ok(NetworkInfo {
            id: id,
            tcp_stream,
            key,
        })
    } else {
        Err(Error::new(ErrorKind::Other, "Failed to connect to server"))
    }
}
