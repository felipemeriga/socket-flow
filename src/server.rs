use std::io;
use tokio::net::TcpListener;
use crate::handshake::perform_handshake;

// pub struct Server {
//
// }

pub async fn run() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let result = perform_handshake(socket).await;
            match result {
                Ok(mut ws_connection) => {
                    while let Some(message) = ws_connection.read.recv().await {
                        // println!("GOT = {}", String::from_utf8(message).unwrap());
                        ws_connection.write.send(message).unwrap();
                    }
                    println!("stopped receiving updates")
                }
                Err(e) => {
                    println!("failed to read from connection: {}", e);
                }
            }
        });
    }
}