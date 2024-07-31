use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::time::timeout;
use crate::error::{CloseError, StreamError};
use crate::frame::{Frame, OpCode};

pub struct WSConnection {
    pub read: Receiver<Result<Vec<u8>, StreamError>>,
    pub write: Sender<Frame>,
    close_rx: Receiver<bool>
}

impl WSConnection {
    pub fn new(read: Receiver<Result<Vec<u8>, StreamError>>, write: Sender<Frame>, close_rx: Receiver<bool>) -> Self {
        Self { read, write, close_rx }
    }

    pub async fn close_connection(&mut self) -> Result<(), CloseError>{
        self.write.send(Frame::new(true, OpCode::Close, Vec::new())).await?;

        match timeout(Duration::from_millis(200), self.close_rx.recv()).await {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Ok(()),
            Err(err) => Err(err)?,
        }
    }

    // TODO - Create method for binary
    pub async fn send_data(&mut self, data: Vec<u8>) -> Result<(), SendError<Frame>>{
        self.write.send(Frame::new(true, OpCode::Text, data)).await
    }

    pub async fn send_ping(&mut self) -> Result<(), SendError<Frame>> {
        self.write.send(Frame::new(true, OpCode::Ping, Vec::new())).await
    }
}