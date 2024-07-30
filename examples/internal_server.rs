use std::net::SocketAddr;
use log::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use simple_websocket::frame::{Frame, OpCode};
use simple_websocket::handshake::perform_handshake;

async fn handle_connection(_: SocketAddr, stream: TcpStream) {
    match perform_handshake(stream).await {
        Ok(mut ws_connection) => {
            loop {
                select! {
                    Some(result) = ws_connection.read.recv() => {
                        match result {
                            Ok(message) => {
                                if ws_connection.write.send(Frame::new(true, OpCode::Text, message)).await.is_err() {
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
                    else => println!("Connection dropped")
                }
            }
        }
        Err(err) => eprintln!("Error when performing handshake: {}", err)
    }
    println!("closed connection");
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