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
                            eprintln!("Failed to send message");
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Received error from the stream: {}", e);
                        break;
                    }
                }
            }
        }
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
