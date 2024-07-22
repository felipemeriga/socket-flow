use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct WSConnection {
    pub read: UnboundedReceiver<Vec<u8>>,
    pub write: UnboundedSender<Vec<u8>>,
}

impl WSConnection {
    pub fn new(read: UnboundedReceiver<Vec<u8>>, write: UnboundedSender<Vec<u8>>) -> Self {
        Self { read, write }
    }
}

pub struct ConnectionPool {
    pub connections: Arc<Mutex<HashMap<SocketAddr, WSConnection>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub async fn add_connection(&self, socket_address: SocketAddr, ws_connection: WSConnection) {
        let mut connections = self.connections.lock().unwrap();
        connections.insert(socket_address, ws_connection);
    }

    pub async fn remove_connection(&self, socket_address: &SocketAddr) -> Option<WSConnection> {
        let mut connections = self.connections.lock().unwrap();
        connections.remove(socket_address)
    }

    // any other methods to manipulate connections...
}