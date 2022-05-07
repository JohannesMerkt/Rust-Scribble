mod network;

fn main() {
    network::connect_to_server("127.0.0.1", 3000);
}
