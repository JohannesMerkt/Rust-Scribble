mod gamestate;
mod network;

use std::{io, thread, time::Duration};

fn main() {
    //Read in a username from the user
    let mut user_input = String::new();
    println!("Enter your name: ");
    io::stdin()
        .read_line(&mut user_input)
        .expect("error: unable to read user input");

    //Create a new net_info object
    let res = network::connect_to_server("127.0.0.1", 3000, &user_input.trim());

    match res {
        Ok(mut net_info) => {
            loop {
                let rcv_msg = network::read_tcp_message(&mut net_info);
                match rcv_msg {
                    Ok(msg) => {
                        //Handle Messages here
                        //For Example, update gamestate, update chat, etc.
                        println!("{:?}", msg);
                    }
                    Err(_) => { /*Read Errors could be handled here*/ }
                }

                //Can use snd_res to detect server disconnection
                let snd_res = network::send_chat_message(&mut net_info, "Hello world!");
                match snd_res {
                    Ok(_) => {}
                    Err(_) => {
                        println!("Server Disconnected");
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
        Err(e) => panic!("Failed to connect to server {}", e),
    };
}
