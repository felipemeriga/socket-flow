# Executing The Library With Additional Configuration and Extensions

## Introduction

Apart from the common methods for connecting and accepting connections straightaway, 
we also offer the methods `accept_async_with_config` for websocket servers, and
`connect_async_with_config`.
Where the end-user can configure some websockets parameters, enable extensions for message
compression and decompression, and even adding custom TLS certificates.

## Configurations

In this library
for server and client config we offer the following parameters, which are all optional:
- `max_frame_size`: Maximum value for Frame payload size, not counting the underlying basic frame components.
- `max_message_size`: Maximum payload size a message can have.
- `extensions`:
  - `permessage_deflate`: Dictates if compression is enabled.
  - `client_no_context_takeover`: Asks that the client should reset its compression context after compressing a message.
  - `server_no_context_takeover`: Asks that the server should reset its compression context after compressing a message.
  - `client_max_window_bits`: Asks that the client sets its compression window to a specific number.
  - `server_max_window_bits`: Asks that the client sets its compression window to a specific number.

## Examples

Here we are going to show how can you setup a server and a client, configuring some parameters and enabling
compression and decompression:

### Server:
```rust
use futures::StreamExt;
use log::*;
use socket_flow::config::WebSocketConfig;
use socket_flow::extensions::Extensions;
use socket_flow::handshake::accept_async_with_config;
use socket_flow::stream::SocketFlowStream;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

async fn handle_connection(_: SocketAddr, stream: TcpStream) {
    let mut config = WebSocketConfig::default();
    config.extensions = Some(Extensions {
        permessage_deflate: true,
        client_no_context_takeover: Some(true),
        server_no_context_takeover: Some(true),
        client_max_window_bits: None,
        server_max_window_bits: None,
    });

    match accept_async_with_config(SocketFlowStream::Plain(stream), Some(config)).await {
        Ok(mut ws_connection) => {
            while let Some(result) = ws_connection.next().await {
                match result {
                    Ok(message) => {
                        if ws_connection.send_message(message).await.is_err() {
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

    while let Ok((stream, peer)) = listener.accept().await {
        info!("Peer address: {}", peer);
        tokio::spawn(handle_connection(peer, stream));
    }
}
```

### Client:
```rust
use futures::StreamExt;
use log::*;
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use socket_flow::handshake::{connect_async, connect_async_with_config};
use tokio::select;
use tokio::time::{interval, Duration};
use socket_flow::config::{ClientConfig, WebSocketConfig};
use socket_flow::extensions::Extensions;

fn get_config() -> ClientConfig {
    let mut websocket_config = WebSocketConfig::default();
    websocket_config.extensions = Some(Extensions {
        permessage_deflate: true,
        client_no_context_takeover: Some(true),
        server_no_context_takeover: Some(true),
        client_max_window_bits: None,
        server_max_window_bits: None,
    });
    let mut client_config = ClientConfig::default();
    client_config.web_socket_config = websocket_config;
    client_config
}

async fn handle_connection(addr: &str) {
    let client_config = get_config();
    
    match connect_async_with_config(addr, Some(client_config)).await {
        Ok(mut ws_connection) => {
            let mut ticker = interval(Duration::from_secs(5));
            // it will be used for closing the connection
            let mut counter = 0;

            loop {
                select! {
                    Some(result) = ws_connection.next() => {
                        match result {
                            Ok(message) => {
                                 info!("Received message: {}", message.as_text().unwrap());
                                counter = counter + 1;
                                // close the connection if 3 messages have already been sent and received
                                if counter >= 3 {
                                    if ws_connection.close_connection().await.is_err() {
                                         error!("Error occurred when closing connection");
                                    }
                                    break;
                                }
                            }
                            Err(err) => {
                                error!("Received error from the stream: {}", err);

                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        let random_string = generate_random_string();
                        let binary_data = Vec::from(random_string);

                        if ws_connection.send(binary_data).await.is_err() {
                            eprintln!("Failed to send message");
                            break;
                        }
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
    handle_connection("ws://localhost:9002").await;
}

fn generate_random_string() -> String {
    rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}
```

## Compression and Decompression

When you enable `permessage_deflate` extension over the config, your server or client, will be capable
of handling compression and decompression of payloads.
Since this is a websocket extension,
your server/client will always negotiate the extensions, if the other third party that is trying to connect to your server,
or the server you are trying to connect.

By default, if compression is enabled over a connection,
this library will automatically compress all the payloads that are bigger than 8KB.