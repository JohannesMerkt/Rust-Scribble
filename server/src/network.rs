use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use json::object;
use json::JsonValue;
use rand::Rng;
use rand_core::OsRng;
use std::io::{Error, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::thread::sleep;
use x25519_dalek::{EphemeralSecret, PublicKey};

pub struct NetworkInfo {
    username: String,
    stream: TcpStream,
    shared_secret: Key,
}

fn generate_keypair() -> (PublicKey, EphemeralSecret) {
    let secret = EphemeralSecret::new(OsRng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

pub fn tcp_server() -> Result<(), Error> {
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(loopback, 3000);
    let listener = TcpListener::bind(socket)?;
    let port = listener.local_addr()?;

    let (public_key, secret_key) = generate_keypair();
    println!("Listening on {}, access this port to end the program", port);

    //TODO accept connection and start a thread to handle it
    let (mut tcp_stream, addr) = listener.accept()?;
    println!("Connection received! {:?} is Connected.", addr);

    tcp_stream.write_all(public_key.as_bytes())?;
    handle_client(tcp_stream, secret_key);
    Ok(())
}

fn encrypt_json(json_message: JsonValue, shared_key: Key) -> (usize, Nonce, Vec<u8>) {
    let nonce = *Nonce::from_slice(rand::thread_rng().gen::<[u8; 12]>().as_slice());
    let cipher = ChaCha20Poly1305::new(&shared_key);

    let ciphertext = cipher
        .encrypt(&nonce, json_message.dump().as_bytes())
        .expect("encryption failure!");

    let msg_size = ciphertext.len() + 12;
    //prefix the nonce to the ciphertext

    (msg_size, nonce, ciphertext)
}

fn send_tcp_message(tcp_stream: &mut TcpStream, net_msg: (usize, Nonce, Vec<u8>)) {
    //TODO send 1 message not 3
    let _ = tcp_stream.write(&usize::to_le_bytes(net_msg.0));
    let _ = tcp_stream.write(&net_msg.1);
    let _ = tcp_stream.write(&net_msg.2);
}

pub fn send_game_state(net_info: &mut NetworkInfo) {
    //TODO get shared game state (don't create it here)
    let game_state = object! {
        code: 100,
        payload: {
            users: {
                "Bob": 10,
                "Sally": 0,
                "Alice": 10
            },
            turn: "Bob",
            image:[[10, 5],[11, 5],[12, 5]]
        }
    };

    let net_msg = encrypt_json(game_state, net_info.shared_secret);
    send_tcp_message(&mut net_info.stream, net_msg);
}

fn handle_client(mut tcp_stream: TcpStream, secret_key: EphemeralSecret) {
    let mut buffer = [0; 32];
    let _ = tcp_stream.read(&mut buffer);

    let client_public: PublicKey = PublicKey::from(buffer);
    let shared_secret = secret_key.diffie_hellman(&client_public);
    let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());

    //TODO get username from client
    let mut net_info = NetworkInfo {
        username: "Bob".to_string(),
        stream: tcp_stream,
        shared_secret: key,
    };

    loop {
        send_game_state(&mut net_info);
        sleep(std::time::Duration::from_secs(1));
    }
}
