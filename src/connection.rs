use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct WSConnection {
    read: UnboundedReceiver<Vec<u8>>,
    write: UnboundedSender<Vec<u8>>
}

impl WSConnection {
    pub fn new(read: UnboundedReceiver<Vec<u8>>, write: UnboundedSender<Vec<u8>>) -> Self {
        Self { read, write }
    }
}


