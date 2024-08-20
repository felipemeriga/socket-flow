# socket-flow

Simple async WebSockets implementation for Tokio stack.

[![Apache licensed](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/felipemeriga/socket-flow/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/v/socket-flow.svg)](https://crates.io/crates/socket-flow)

## Introduction

This library is supposed to offer a simple implementation for websockets, so end-user could use this
to wrap a websockets server/client into their application, offering a smooth way of setting it up into his code.

It's an async library based on tokio runtime,
which uses a tokio TcpStream behind the scenes, using that as the starting point
to implement the standards of [WebSocket Protocol RFC](https://datatracker.ietf.org/doc/html/rfc6455),
performing handshakes, reading frames, parsing masks, handling opcodes and internal payload.

Can be used as a client or server,
returning a `WSConnection`, which implements the `Stream` trait,
so you can continuously consume incoming messages, or send messages.

The motivation behind this, was to offer a simple way of having a WebSockets connection over your application, using as a 
reference wide established libraries, like  [`tungstenite-rs`](https://github.com/snapview/tungstenite-rs) and [`tokio-tungstenite`](https://github.com/snapview/tokio-tungstenite/tree/master)

## Features

Most of all WebSockets RFC features are implemented, like:
- Handshake process, key parsing and generation
- OpCodes handling, like `Text`, `Binary`, `Ping`, `Pong` and `Continue`
- Multiple subscriptions
- Scalability
- Error handling
- It passes the autobahn-test-suite

Features to be added:
- TLS/SSL support
- Compression

## Usage

Add this in your `Cargo.toml`:

```toml
[dependencies]
socket-flow = "*"
```

## Example of usage

### Echo server
Here is a echo-server example, that you can also find in: [Example](./examples/internal_server.rs)

```rust
use futures::StreamExt;
use log::*;
use socket_flow::handshake::accept_async;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

async fn handle_connection(_: SocketAddr, stream: TcpStream) {
    match accept_async(stream).await {
        Ok(mut ws_connection) => {
            while let Some(result) = ws_connection.next().await {
                match result {
                    Ok(frame) => {
                        if ws_connection.send_frame(frame).await.is_err() {
                            error!("Failed to send message");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Received error from the stream: {}", e);
                        break;
                    }
                }
            }
        }
        Err(err) => error!("Error when performing handshake: {}", err),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(handle_connection(peer, stream));
    }
}
```

For running this example, you can clone the repo and execute:
```shell
cargo run --color=always --package socket-flow --example internal_server
```

This example, creates a TcpListener, binding it to a port, accepting connections, handling each of these connections
inside a tokio task, for handling clients concurrently. The handle_connection function, make sure the handshake process
is performed, returning a `WSConnection`, which implements `Stream` trait, where you can consume incoming data for this client, 
and perform write operations into the socket.
It includes error handling through `Result`.

### Simple client

Here is an example of how to run a client, that will perform some operations and disconnect gracefully:
```rust
use futures::StreamExt;
use rand::distr::Alphanumeric;
use rand::{thread_rng, Rng};
use socket_flow::handshake::connect_async;
use tokio::select;
use tokio::time::{interval, Duration};

async fn handle_connection(addr: &str) {
    match connect_async(addr).await {
        Ok(mut ws_connection) => {
            let mut ticker = interval(Duration::from_secs(5));
            // it will be used for closing the connection
            let mut counter = 0;

            loop {
                select! {
                    Some(result) = ws_connection.next() => {
                        match result {
                            Ok(frame) => {
                                 println!("Received message: {}", &String::from_utf8(frame.payload).unwrap());
                                counter = counter + 1;
                                // close the connection if 3 messages have already been sent and received
                                if counter >= 3 {
                                    if ws_connection.close_connection().await.is_err() {
                                         eprintln!("Error occurred when closing connection");
                                    }
                                    break;
                                }
                            }
                            Err(err) => {
                                eprintln!("Received error from the stream: {}", err);

                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        let random_string = generate_random_string();
                        let binary_data = Vec::from(random_string);

                        if ws_connection.send_data(binary_data).await.is_err() {
                            eprintln!("Failed to send message");
                            break;
                        }
                    }
                }
            }
        }
        Err(err) => eprintln!("Error when performing handshake: {}", err),
    }
}

#[tokio::main]
async fn main() {
    handle_connection("ws://127.0.0.1:9002").await;
}

fn generate_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}
```

Since you need a server for testing the client, you can execute our echo-server example, and on another tab
execute the client example:
```shell
cargo run --color=always --package socket-flow --example client
```

In this example, the client will try to connect to `ws://127.0.0.1:9002`,
if the connection is established, it will start sending random strings every 5 seconds into the socket.
After sending three strings, it will close the connection gracefully and end its execution.


You can check more examples over [Examples](./examples)

## Testing

Socket-flow passes the [Autobahn Test Suite](https://github.com/crossbario/autobahn-testsuite) for
WebSockets.
Also, it has some internal tests, for ensuring reliability

## References

- [`tungstenite-rs`](https://github.com/snapview/tungstenite-rs)
- [`tokio-tungstenite`](https://github.com/snapview/tokio-tungstenite/tree/master)



