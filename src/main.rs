mod handshake;
mod stream;
mod frame;
mod connection;

use tokio::net::{TcpListener};
use std::{io};
use crate::handshake::perform_handshake;

#[tokio::main]
pub async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let result = perform_handshake(socket).await;
            match result {
                Ok(_) => {}
                Err(_) => {}
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