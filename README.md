# socket-flow

Straightforward async WebSocket library for Rust! With plenty of examples available!

[![Apache licensed](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/felipemeriga/socket-flow/blob/main/LICENSE)  
[![Crates.io](https://img.shields.io/crates/v/socket-flow.svg)](https://crates.io/crates/socket-flow)

---

## Introduction

`socket-flow` is a lightweight library designed to simplify the implementation of WebSocket connections in Rust. Whether you need a WebSocket server or client, this library provides an easy-to-use and efficient solution.

Built on top of the asynchronous `tokio` runtime, `socket-flow` leverages `tokio::TcpStream` to implement the standards of the [WebSocket Protocol RFC 6455](https://datatracker.ietf.org/doc/html/rfc6455). It handles:

- WebSocket handshakes
- Frame reading and parsing
- Mask and opcode handling
- Payload management

This library was inspired by well-established WebSocket libraries such as [`tungstenite-rs`](https://github.com/snapview/tungstenite-rs) and [`tokio-tungstenite`](https://github.com/snapview/tokio-tungstenite). Its goal is to offer a more accessible way to integrate WebSocket connections into applications while maintaining flexibility and performance.

---

## Features

`socket-flow` implements most features defined in the WebSocket RFC:

- Handshake process with key parsing and generation
- Opcode handling (`Text`, `Binary`, `Ping`, `Pong`, and `Continuation` frames)
- Multi-client support and scalability
- Robust error handling
- Passes the [Autobahn Test Suite](https://github.com/crossbario/autobahn-testsuite)
- TLS support via [tokio-rustls](https://github.com/rustls/tokio-rustls)
- Extensions for compression and decompression using permessage-deflate

---

## Getting Started

Add `socket-flow` to your `Cargo.toml` dependencies:

```toml
[dependencies]
socket-flow = "*"
```

---

## Usage Examples

The repository includes several examples showcasing different ways to use `socket-flow`. It provides both a "plug-and-play" server setup and more customizable approaches, allowing developers to tailor the library to their needs.

### Plug-and-Play Server

With the `start_server` function, you can quickly spin up a WebSocket server. The function returns an `EventStream`, which allows you to consume server events like new connections, messages, errors, and disconnections.

#### Code Example:

```rust
use futures::StreamExt;
use log::*;
use socket_flow::event::{Event, ID};
use socket_flow::server::start_server;
use socket_flow::split::WSWriter;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    env_logger::init();

    let port: u16 = 8080;
    match start_server(port).await {
        Ok(mut event_receiver) => {
            let mut clients: HashMap<ID, WSWriter> = HashMap::new();
            info!("Server started on address 127.0.0.1:{}", port);
            while let Some(event) = event_receiver.next().await {
                match event {
                    Event::NewClient(id, client_conn) => {
                        info!("New client {} connected", id);
                        clients.insert(id, client_conn);
                    }
                    Event::NewMessage(client_id, message) => {
                        info!("Message from client {}: {:?}", client_id, message);
                        if let Some(ws_writer) = clients.get_mut(&client_id) {
                            ws_writer.send_message(message).await.unwrap();
                        }
                    }
                    Event::Disconnect(client_id) => {
                        info!("Client {} disconnected", client_id);
                        clients.remove(&client_id);
                    }
                    Event::Error(client_id, error) => {
                        error!("Error occurred for client {}: {:?}", client_id, error);
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("Could not start the server: {:?}", err);
        }
    }
}
```

Run this example with:

```sh
cargo run --example simple_server
```

---

### Echo Server

The echo server accepts incoming connections, performs the WebSocket handshake, and echoes back messages.

#### Code Example:

```rust
use futures::StreamExt;
use log::*;
use socket_flow::handshake::accept_async;
use socket_flow::stream::SocketFlowStream;
use tokio::net::{TcpListener, TcpStream};

async fn handle_connection(stream: TcpStream) {
    if let Ok(mut ws_connection) = accept_async(SocketFlowStream::Plain(stream)).await {
        while let Some(result) = ws_connection.next().await {
            match result {
                Ok(message) => {
                    if ws_connection.send_message(message).await.is_err() {
                        error!("Failed to send message");
                        break;
                    }
                }
                Err(err) => {
                    error!("Stream error: {}", err);
                    break;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(stream));
    }
}
```

Run this example with:

```sh
cargo run --example echo_server
```

---

### Simple Client

A client example demonstrates sending messages and gracefully closing the connection after three exchanges.

#### Code Example:

```rust
use futures::StreamExt;
use log::*;
use socket_flow::handshake::connect_async;
use tokio::time::{interval, Duration};

async fn handle_connection(addr: &str) {
    if let Ok(mut ws_connection) = connect_async(addr).await {
        let mut ticker = interval(Duration::from_secs(5));
        let mut counter = 0;

        loop {
            tokio::select! {
                Some(result) = ws_connection.next() => {
                    match result {
                        Ok(message) => {
                            info!("Received: {:?}", message);
                            counter += 1;
                            if counter >= 3 {
                                ws_connection.close_connection().await.unwrap();
                                break;
                            }
                        }
                        Err(err) => {
                            error!("Error: {}", err);
                            break;
                        }
                    }
                }
                _ = ticker.tick() => {
                    let msg = "Hello, WebSocket!".to_string();
                    ws_connection.send(msg.into_bytes()).await.unwrap();
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    handle_connection("ws://127.0.0.1:9002").await;
}
```

Run this example with:

```sh
cargo run --example client
```

---

## Testing

- The library passes the [Autobahn Test Suite](https://github.com/crossbario/autobahn-testsuite) for WebSocket compliance.
- Internal tests ensure reliability.

---

## TLS/SSL Support

TLS is supported via [tokio-rustls](https://github.com/rustls/tokio-rustls).

Find setup details in the [TLS Examples](https://github.com/felipemeriga/socket-flow/blob/main/TLS.md).

---

## Configuration and Compression

Learn how to configure WebSocket parameters and enable compression/decompression via permessage-deflate in the [Config and Extensions Guide](https://github.com/felipemeriga/socket-flow/blob/main/CONFIG.md).
Compared to another libraries, where you need to configure compression by yourself, you just need to activate it through this
library config.

---

## References

- [`tungstenite-rs`](https://github.com/snapview/tungstenite-rs)
- [`tokio-tungstenite`](https://github.com/snapview/tokio-tungstenite)
- [`tokio-rustls`](https://github.com/rustls/tokio-rustls)

---

Here’s a new section for the `README.md` highlighting why developers should use `socket-flow` and how it differentiates itself from other libraries like `tokio-tungstenite`:

---

## Why Choose Socket-Flow?

When developing WebSocket-based applications, ease of use and feature completeness are crucial factors in selecting the right library. Here's why `socket-flow` stands out compared to other libraries like `tokio-tungstenite`:

### 1. **Ease of Setup**
Unlike many WebSocket libraries that require complex configurations or additional setup, `socket-flow` provides a developer-friendly interface to get started quickly. Whether you're creating a WebSocket server or client, `socket-flow` offers both "plug-and-play" and fully configurable approaches to fit your needs.

- **Plug-and-Play:** Get a WebSocket server running with minimal lines of code using the `start_server` function, which handles connections, messages, errors, and disconnections for you.
- **Fully Configurable:** For advanced use cases, `socket-flow` allows you to customize your WebSocket setup while still benefiting from its comprehensive features.

### 2. **Built-In Compression/Decompression**
WebSocket compression is essential for reducing bandwidth and optimizing data transfer. While other libraries like `tokio-tungstenite` support WebSocket compression, they often only allow basic context management between messages (reset or reuse), requiring you to implement additional logic for advanced configurations.

`socket-flow` simplifies this by offering built-in support for **permessage-deflate** compression and decompression. All you need to do is provide a configuration, and the library takes care of the rest. You can easily configure parameters such as:

- Compression level
- Memory usage
- Context resetting or keeping between messages

This out-of-the-box support saves time and eliminates the need for extending or modifying the library yourself.
