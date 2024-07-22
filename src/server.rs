use std::io;
use std::sync::{Arc};
use tokio::net::TcpListener;
use crate::connection::{ConnectionPool};
use crate::handshake::perform_handshake;

pub struct Server {
    port: i64,
    pub connection_pool: Arc<ConnectionPool>,
    pub handle: Option<tokio::task::JoinHandle<io::Result<()>>>,
}

impl Server {
    pub async fn new(port: i64) -> Self {
        let mut server = Self {
            port,
            connection_pool: Arc::new(ConnectionPool::new()),
            handle: None,
        };
        server.run_server().await.expect("TODO: panic message");
        server
    }

    pub async fn run_server(&mut self) -> io::Result<()> {
        let port = self.port.clone();
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        let pool = Arc::clone(&self.connection_pool);

        let handle = tokio::spawn(async move {
            loop {
                let (socket, socket_address) = listener.accept().await?;
                let conn_pool = Arc::clone(&pool);

                tokio::spawn(async move {
                    let result = perform_handshake(socket).await;
                    match result {
                        Ok(ws_connection) => {
                            conn_pool.add_connection(socket_address, ws_connection).await;
                        }
                        Err(e) => println!("Failed to read from the connection: {}", e),
                    }
                });
            }
        });
        self.handle = Some(handle);
        Ok(())
    }
}