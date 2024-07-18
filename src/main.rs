mod connection;
mod frame;
mod handshake;
mod read;
mod write;

use crate::handshake::perform_handshake;
use std::io;
use tokio::net::TcpListener;

#[tokio::main]
pub async fn main() -> io::Result<()> {
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

// async fn read_all_from_stream(mut stream: TcpStream) -> Result<Vec<u8>, Box<dyn Error>> {
//     // Define an empty buffer to store the results
//     let mut response = Vec::new();
//
//     // TCP is a stream-based protocol. That means that we need to keep reading into
//     // the buffer until the other side closes the connection.
//     stream.read_to_end(&mut response).await?;
//
//     Ok(response)
// }
