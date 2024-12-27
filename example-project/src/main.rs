use clap::Parser;
use serde_dispatch::serde_dispatch;
use std::os::unix::net::{UnixListener, UnixStream};

// the interface that is used for RPC
#[serde_dispatch]
trait World {
    fn ping(&self, num: usize) -> String;
}

// the implementation of what happens on the server for remote calls
struct ConcreteWorld;
impl World for ConcreteWorld {
    fn ping(&self, num: usize) -> String {
        println!("SERVER: function `ping` got called with num={num}");
        return "pong ".repeat(num).into();
    }
}

// offers two modes which communicate via unix sockets:
// `cargo run server` will run the server side
// `cargo run client` will run a client that makes a call to `ping()` once
// you should first start the server and then run the client from another shell
// NOTE: when you restart the server, make sure to delete the `mysocket` file in between
// (otherwise it will fail to bind the UnixListener and panic with "address already in use")
fn main() {
    let args = Args::parse();
    match args {
        Args::Server => run_server(),
        Args::Client => run_client(),
    }
}

fn run_server() {
    let mut world = ConcreteWorld;
    let listener = UnixListener::bind("mysocket").unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("SERVER: new connection! will handle a single request");
                // handles a single remote procedure call
                world.handle_with(&stream, &stream).unwrap();
                stream.shutdown(std::net::Shutdown::Both).unwrap();
                println!("SERVER: request done, will drop connection");
            }
            Err(err) => {
                panic!("{err}")
            }
        }
    }
}

fn run_client() {
    let stream = UnixStream::connect("mysocket").unwrap();
    println!("CLIENT: connected! will make a single request to call `ping` with num=5");
    let result = WorldRPCClient::call_with(&stream, &stream)
        .ping(&5)
        .unwrap();
    stream.shutdown(std::net::Shutdown::Both).unwrap();
    println!("CLIENT: request done with the following return value: (next line)\n{result}");
}

#[derive(Parser)]
enum Args {
    Server,
    Client,
}
