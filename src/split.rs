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
use crate::config::WebSocketConfig;

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
    web_socket_config: WebSocketConfig
}

impl WSWriter {
    pub fn new(writer: Arc<Mutex<Writer>>, web_socket_config: WebSocketConfig) -> Self {
        Self { writer, web_socket_config }
    }

    pub async fn close_connection(&mut self) -> Result<(), Error> {
        self.write_frames(vec![Frame::new(true, OpCode::Close, Vec::new())])
            .await?;
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    pub async fn send_message(&mut self, message: Message) -> Result<(), Error> {
        self.write_message(message).await
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.write_message(Message::Text(String::from_utf8(data)?)).await
    }

    pub async fn send_as_binary(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.write_message(Message::Binary(data))
            .await
    }

    pub async fn send_as_text(&mut self, data: String) -> Result<(), Error> {
        self.write_message(Message::Text(data)).await
    }

    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.write_frames(vec![Frame::new(true, OpCode::Ping, Vec::new())])
            .await
    }

    pub async fn send_large_data_fragmented(&mut self, data: Vec<u8>, fragment_size: usize) -> Result<(), Error> {
        // Each fragment size will be limited by max_frame_size config,
        // that had been given by the user,
        // or it will use the default max frame size which is 16 mb.
        if fragment_size > self.web_socket_config.max_frame_size.unwrap_or_default() {
            return Err(Error::CustomFragmentSizeExceeded(fragment_size, self.web_socket_config.max_frame_size.unwrap_or_default()));
        }

        if data.len() > self.web_socket_config.max_message_size.unwrap_or_default() {
            return Err(Error::MaxMessageSize);
        }

        let chunks = data.chunks(fragment_size);
        let total_chunks = chunks.len();

        for (i, chunk) in chunks.enumerate() {
            let is_final = i == total_chunks - 1;
            let opcode = if i == 0 {
                OpCode::Text
            } else {
                OpCode::Continue
            };

            self.write_frames(vec![Frame::new(is_final, opcode, Vec::from(chunk))])
                .await?
        }

        Ok(())
    }

    async fn write_message(&mut self, message: Message) -> Result<(), Error> {
        if message.as_binary().len() > self.web_socket_config.max_message_size.unwrap_or_default() {
            return Err(Error::MaxMessageSize);
        }

        self.write_frames(message.to_frames(self.web_socket_config.max_frame_size.unwrap_or_default())).await
    }

    async fn write_frames(&mut self, frames: Vec<Frame>) -> Result<(), Error> {
        for frame in frames {
            self.writer.lock().await.write_frame(frame).await?
        }
        Ok(())
    }
}
