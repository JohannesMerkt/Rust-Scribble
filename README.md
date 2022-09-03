# Rust Scribble Game README

A client/server scribble game written in Rust.
### Features
 * Draw in multiple colours and sizes and erase mistakes easily
 * Chat between users and correctly guessed words are not shown
 * Concurrent lobbies/games on a single server
 * Configurable words and gameplay
 * Server port is configurable
 * Network connection is end-to-end encrypted by default 

---
## Starting a Server

To Run the server, enter the server directory and use cargo run to start.
    
``` bash
cargo run
```
### Configuration parameters
The server can be configured on the command line. They can be used individually or in any combination

The options available are
* ```--port``` : Port number 
* ```--words```: Word list file
* ```--time```: Length of game (seconds)

To use the command line configuration simply use
```bash
cargo run -- --port 4001 --words filepath/filename.txt
```

Servers can also be configured to run without end-to-end encryption, using the conditional compilation feature
```bash
cargo run --features no-encryption
```
**Note that all clients connecting will also need to be compiled with this features in order to communicate properly**

----
## Running a Client

Open new terminal and change directory to client. Then run  
```bash 
cargo run 
``` 

To create a client without end-to-end encryption, use the same command as the server.
```bash
cargo run --features no-encryption
```