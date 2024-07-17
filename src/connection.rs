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
