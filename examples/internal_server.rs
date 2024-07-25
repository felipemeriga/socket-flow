use std::net::SocketAddr;
use log::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use simple_websocket::handshake::perform_handshake;

async fn handle_connection(_: SocketAddr, stream: TcpStream) {
    match perform_handshake(stream).await {
        Ok(mut ws_connection) => {
            loop {
                select! {
                    Some(message) = ws_connection.read.recv() => {
                        if ws_connection.write.send(message).is_err() {
                            eprintln!("Failed to send message");
                            break;
                        }
                    }
                    Some(error) = ws_connection.errors.recv() => {
                        eprintln!("Received error from the stream: {}", error);
                        break
                    }
                    else => break
                }
            }
        }
        Err(err) => eprintln!("Error when performing handshake: {}", err)
    }
}


#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);


    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr().expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(handle_connection(peer, stream));
    }
}