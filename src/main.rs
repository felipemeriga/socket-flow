mod connection;
mod frame;
mod handshake;
mod read;
mod write;
mod server;

use std::io;
use crate::server::{Server};

#[tokio::main]
pub async fn main() -> io::Result<()> {
    Server::new(9000).run_server().await
}