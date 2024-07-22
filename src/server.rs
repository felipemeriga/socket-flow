use std::io;
use tokio::net::TcpListener;
use crate::handshake::perform_handshake;

pub struct Server {
    port: i64
}

impl Server {
    pub fn new(port: i64) -> Self {
        Self { port }
    }

    pub async fn run_server(&mut self) -> io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port)).await?;

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
}