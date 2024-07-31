use crate::error::{CloseError, StreamError};
use crate::frame::{Frame, OpCode};
use std::time::Duration;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::timeout;

const CLOSE_TIMEOUT: u64 = 5;
pub struct WSConnection {
    pub read: Receiver<Result<Vec<u8>, StreamError>>,
    write: Sender<Frame>,
    close_rx: Receiver<bool>,
}

impl WSConnection {
    pub fn new(
        read: Receiver<Result<Vec<u8>, StreamError>>,
        write: Sender<Frame>,
        close_rx: Receiver<bool>,
    ) -> Self {
        Self {
            read,
            write,
            close_rx,
        }
    }

    pub async fn close_connection(&mut self) -> Result<(), CloseError> {
        self.write
            .send(Frame::new(true, OpCode::Close, Vec::new()))
            .await?;

        match timeout(Duration::from_secs(CLOSE_TIMEOUT), self.close_rx.recv()).await {
            Err(err) => Err(err)?,
            _ => Ok(()),
        }
    }

    // TODO - Create method for binary
    pub async fn send_data(&mut self, data: Vec<u8>) -> Result<(), SendError<Frame>> {
        self.write.send(Frame::new(true, OpCode::Text, data)).await
    }

    pub async fn send_ping(&mut self) -> Result<(), SendError<Frame>> {
        self.write
            .send(Frame::new(true, OpCode::Ping, Vec::new()))
            .await
    }
}
