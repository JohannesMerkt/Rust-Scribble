use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use generic_array::GenericArray;
use rand_core::OsRng;
use std::io::{Error, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::str;
use x25519_dalek::{EphemeralSecret, PublicKey};

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

    let (mut tcp_stream, addr) = listener.accept()?;
    println!("Connection received! {:?} is Connected.", addr);

    tcp_stream.write_all(public_key.as_bytes())?;
    handle_client(tcp_stream, secret_key);
    Ok(())
}

fn handle_client(mut tcp_stream: TcpStream, secret_key: EphemeralSecret) {
    let mut buffer = [0; 32];
    let _ = tcp_stream.read(&mut buffer);

    let client_public: PublicKey = PublicKey::from(buffer);
    let shared_secret = secret_key.diffie_hellman(&client_public);

    let mut msg_buf = [0; 1024];
    let read_size = tcp_stream.read(&mut msg_buf);

    let key: chacha20poly1305::Key = *Key::from_slice(shared_secret.as_bytes());
    let cipher = ChaCha20Poly1305::new(&key);
    let nonce: Nonce = GenericArray::clone_from_slice(&msg_buf[0..12]);

    match read_size {
        Ok(size) => {
            let ciphertext = &msg_buf[12..size];
            println!("Ciphertext: {:?}", ciphertext);
            let recv_data: String = String::from_utf8(cipher.decrypt(&nonce, ciphertext).unwrap())
                .expect("Invalid UTF-8 sequence");
            let json_message = json::parse(&recv_data);
            println!("{:?}", json_message);
        }
        Err(e) => println!("Error: {}", e),
    }
}
