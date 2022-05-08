use rand_core::OsRng;
use std::io::{Error, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::thread;
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
    let read_size = tcp_stream.read(&mut buffer);

    let client_public: PublicKey = PublicKey::from(buffer);
    println!("Client Key: {:?}", client_public.as_bytes());

    let shared_secret = secret_key.diffie_hellman(&client_public);
    println!("Shared Secret: {:?}", shared_secret.as_bytes());

    let mut msg_buf = String::new();
    let result = tcp_stream.read_to_string(&mut msg_buf);
    let json_message = json::parse(&msg_buf).unwrap();
    println!("{:?}", json_message["payload"]["message"]);
}
