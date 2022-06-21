use std::error;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{Key, ChaCha20Poly1305, Nonce};
use generic_array::GenericArray;
use rand::Rng;
use rand_core::OsRng;
use x25519_dalek::{PublicKey, ReusableSecret};


pub struct NetworkInfo {
    /// The name of the client.
    pub username: String,
    /// The tcp_stream of the client.
    pub tcp_stream: TcpStream,
    /// The public key of the client.
    pub key: Key,
    /// The shared secret of the client and server.
    pub secret_key: Option<ReusableSecret>,
}



/// Verifies if the checksum of the chipher text is correct.
/// 
/// # Arguments
/// * `cipher_text` - The cipher text to be verified.
/// * `checksum` - The checksum to be verified.
/// 
pub fn check_checksum(ciphertext: &[u8], checksum: u32) -> bool {
    checksum == crc32fast::hash(ciphertext)
}

/// Generates a new Public Private keypair.
/// 
/// # Returns
/// * `public_key` - A public key.
/// * `secret_key` - A secret key.
/// 
pub fn generate_keypair() -> (PublicKey, ReusableSecret) {
    let secret = ReusableSecret::new(OsRng);
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
pub fn encrypt_json(json_message: Vec<u8>, shared_key: Key) -> (usize, Nonce, Vec<u8>, u32) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let ciphertext = ChaCha20Poly1305::new(&shared_key).encrypt(&nonce, &json_message[..]).expect("encryption failure!");
    let checksum = crc32fast::hash(&ciphertext);

    //Add 12 bytes for the nonce and 4 bytes for the checksum
    let msg_size = ciphertext.len() + 16;
    (msg_size, nonce, ciphertext, checksum)
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
    res.is_ok() && res.unwrap() > 0
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
pub fn send_tcp_message(
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

/// Reads a tcp_message from the client.
/// 
/// # Arguments
/// * `net_info` - The network information of the client.
/// 
/// # Returns
/// * `Ok(msg)` - The message read from the client in JSON format.
/// * `Err(e)` - The error that occured.
/// 
pub fn read_tcp_message(
     net_info: &mut NetworkInfo,
) -> Result<serde_json::Value, Box<dyn error::Error>> {

    //TODO cleanup and generalize

    let mut size = [0; (usize::BITS / 8) as usize];
    let msg_size;
    let mut msg_buf;
    let cipher;

    net_info.tcp_stream.read_exact(&mut size)?;
    msg_size = usize::from_le_bytes(size);

    msg_buf = vec![0; msg_size];
    net_info.tcp_stream.read_exact(&mut msg_buf)?;
    cipher = ChaCha20Poly1305::new(&net_info.key);

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
        Ok(plaintext) => serde_json::from_slice(&plaintext)?,
        Err(_) => return Err(Box::new(Error::new(ErrorKind::Other, "Decryption failed!"))),
    };

    Ok(json_message)

}