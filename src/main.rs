mod connection;
mod frame;
mod handshake;
mod read;
mod write;
mod server;

use std::io;
use crate::server::run;

#[tokio::main]
pub async fn main() -> io::Result<()> {
    run().await
}