# socket-flow

Simple async WebSockets implementation for Tokio stack.

[![Apache licensed](https://img.shields.io/badge/license-Apache-blue.svg)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/v/socket-flow.svg?maxAge=2592000)](https://crates.io/crates/socket-flow)

## Introduction

This library is supposed to offer a simple implementation for websockets, so end-user could use this
to wrap a websockets server/client into their application, offering a smooth way of setting it up into your code.

It's an async library based on tokio runtime, which takes as argument a tokio TcpStream, using that stream of bytes
to implement the standards of [WebSocket Protocol RFC](https://datatracker.ietf.org/doc/html/rfc6455), performing handshake,
reading frames, parsing masks and internal payload.

Most of the library internal and end-user communication, uses tokio mpsc for passing binary data, frames and errors. After
sending the TcpStream to our function, you receive `WSConnection` struct, which has a read mpsc channel for reading data,
and some public methods for sending binary, text, ping and close frames.

The motivation behind this, was to offer a simple way of having a WebSockets connection over your application, using as a 
reference wide established libraries, like  [`tungstenite-rs`](https://github.com/snapview/tungstenite-rs) and [`tokio-tungstenite`](https://github.com/snapview/tokio-tungstenite/tree/master)

## Features

Most of all WebSockets RFC features are implemented, like:
- Handshake process, key parsing and generation
- OpCodes handling, like `Text`, `Binary`, `Ping`, `Pong` and `Continue`
- Multiple subscriptions
- Scalability
- Error handling

Features to be added:
- Autobahn tests
- TLS/SSL support

## Usage

Add this in your `Cargo.toml`:

```toml
[dependencies]
socket-flow = "*"
```

## Example of usage

Here is a ping-pong server example, that you can also find in: [Example](./examples/internal_server.rs)

```rust
use log::*;
use socket_flow::handshake::perform_handshake;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;

async fn handle_connection(_: SocketAddr, stream: TcpStream) {
    match perform_handshake(stream).await {
        Ok(mut ws_connection) => loop {
            select! {
                Some(result) = ws_connection.read.recv() => {
                    match result {
                        Ok(message) => {
                            if ws_connection.send_data(message).await.is_err() {
                                eprintln!("Failed to send message");
                                break;
                            }
                        }
                        Err(err) => {
                            eprintln!("Received error from the stream: {}", err);
                            break;
                        }
                    }
                }
                else => break
            }
        },
        Err(err) => eprintln!("Error when performing handshake: {}", err),
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
is performed, returning a `WSConnection`, which has a tokio mpsc channel, where you can consume incoming data for this client, 
and perform write into the socket operations, including error handling through `Result`.

You can check more examples over [Examples](./examples)

## References

- [`tungstenite-rs`](https://github.com/snapview/tungstenite-rs)
- [`tokio-tungstenite`](https://github.com/snapview/tokio-tungstenite/tree/master)



