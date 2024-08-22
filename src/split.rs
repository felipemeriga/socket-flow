use crate::error::Error;
use crate::frame::{Frame, OpCode};
use crate::message::Message;
use crate::write::Writer;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;

pub struct WSReader {
    read_rx: ReceiverStream<Result<Message, Error>>,
}

impl WSReader {
    pub fn new(read_rx: ReceiverStream<Result<Message, Error>>) -> Self {
        Self { read_rx }
    }
}

impl Stream for WSReader {
    type Item = Result<Message, Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.read_rx).poll_next(cx)
    }
}

#[derive(Clone)]
pub struct WSWriter {
    writer: Arc<Mutex<Writer>>,
}

impl WSWriter {
    pub fn new(writer: Arc<Mutex<Writer>>) -> Self {
        Self { writer }
    }

    pub async fn close_connection(&mut self) -> Result<(), Error> {
        self.write_frame(Frame::new(true, OpCode::Close, Vec::new()))
            .await?;
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    pub async fn send_message(&mut self, message: Message) -> Result<(), Error> {
        self.write_frame(message.to_frame(true)).await
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.write_frame(Frame::new(true, OpCode::Text, data)).await
    }

    pub async fn send_as_binary(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.write_frame(Frame::new(true, OpCode::Binary, data))
            .await
    }

    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.write_frame(Frame::new(true, OpCode::Ping, Vec::new()))
            .await
    }

    pub async fn send_large_data_fragmented(&mut self, data: Vec<u8>) -> Result<(), Error> {
        const MAX_FRAGMENT_SIZE: usize = 64 * 1024;
        let chunks = data.chunks(MAX_FRAGMENT_SIZE);
        let total_chunks = chunks.len();

        for (i, chunk) in chunks.enumerate() {
            let is_final = i == total_chunks - 1;
            let opcode = if i == 0 {
                OpCode::Text
            } else {
                OpCode::Continue
            };

            self.write_frame(Frame::new(is_final, opcode, Vec::from(chunk)))
                .await?;
        }

        Ok(())
    }

    async fn write_frame(&mut self, frame: Frame) -> Result<(), Error> {
        self.writer.lock().await.write_frame(frame).await
    }
}
