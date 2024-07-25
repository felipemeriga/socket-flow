use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use crate::error::StreamError;

pub struct WSConnection {
    pub read: UnboundedReceiver<Vec<u8>>,
    pub write: UnboundedSender<Vec<u8>>,
    pub errors: UnboundedReceiver<StreamError>
}

impl WSConnection {
    pub fn new(read: UnboundedReceiver<Vec<u8>>, write: UnboundedSender<Vec<u8>>, errors: UnboundedReceiver<StreamError>) -> Self {
        Self { read, write, errors }
    }
}