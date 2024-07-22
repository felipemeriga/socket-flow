use std::io;
use simple_websocket::server::Server;

#[tokio::main]
pub async fn main() -> io::Result<()> {
    let server = Server::new(9000).await;

    loop {
        println!("Doing some work here...");

        // Put the current task to sleep for three seconds.
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
    }
}