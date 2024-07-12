mod handshake;
mod stream;
mod frame;

use tokio::net::{TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use sha1::{Digest};
use std::{io};
use base64::prelude::*;
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