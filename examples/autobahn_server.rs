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
