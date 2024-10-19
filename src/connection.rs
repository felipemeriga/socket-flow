use crate::error::Error;
use crate::frame::{Frame, OpCode};
use crate::message::Message;
use crate::split::{WSReader, WSWriter};
use crate::write::Writer;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use std::vec;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;
use crate::config::WebSocketConfig;

// const CLOSE_TIMEOUT: u64 = 5;
// TODO - Instead of using writer and read_rx, use directly WSRead and WSWrite,
// and return  the struct own attributes when splitting it
pub struct WSConnection {
    writer: Arc<Mutex<Writer>>,
    read_rx: ReceiverStream<Result<Message, Error>>,
    web_socket_config: WebSocketConfig,
}

// WSConnection has the read_rx attribute, which is already a ReceiverStream
// Although, we don't want this attribute visible to the end-user.
// Therefore, implementing Stream for this struct is necessary, so end-user could
// invoke next() and other stream methods directly from a variable that holds this struct.
impl Stream for WSConnection {
    type Item = Result<Message, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // We need to get a mutable reference to the inner field
        let this = self.get_mut();

        // Delegate the polling to `read_rx`
        // We need to pin `read_rx` because its `poll_next` method requires the object to be pinned
        Pin::new(&mut this.read_rx).poll_next(cx)
    }
}

impl WSConnection {
    pub fn new(
        write_half: Arc<Mutex<Writer>>,
        read_rx: ReceiverStream<Result<Message, Error>>,
        web_socket_config: WebSocketConfig,
    ) -> Self {
        Self {
            writer: write_half,
            read_rx,
            web_socket_config,
        }
    }

    /// This function will split the connection into the `WSReader`, which is a stream of messages
    /// and `WSWriter`, for writing data into the socket.
    /// It's a good option when you need to work with both in separate tasks or functions
    pub fn split(self) -> (WSReader, WSWriter) {
        let writer = WSWriter::new(self.writer, self.web_socket_config);
        let reader = WSReader::new(self.read_rx);
        (reader, writer)
    }

    /// This function will be used for closing the connection between two instances, mainly it will
    /// be used by a client,
    /// to request disconnection with a server.It first sends a close frame
    /// through the socket, and waits until it receives the confirmation in a channel
    /// executing it inside a timeout, to avoid a long waiting time
    pub async fn close_connection(&mut self) -> Result<(), Error> {
        self.write_frames(vec![Frame::new(true, OpCode::Close, Vec::new())])
            .await?;

        sleep(Duration::from_millis(500)).await;

        Ok(())

        // match timeout(Duration::from_secs(CLOSE_TIMEOUT), self.write.lock().await.closed()).await {
        //     Err(err) => Err(err)?,
        //     _ => Ok(()),
        // }
    }

    pub async fn send_message(&mut self, message: Message) -> Result<(), Error> {
        self.write_message(message).await
    }

    // This function will be used to send general data as a Vector of bytes, and by default will
    // be sent as a text opcode
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


    // It will send a ping frame through the socket
    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.write_frames(vec![Frame::new(true, OpCode::Ping, Vec::new())])
            .await
    }

    // This function can be used to send large payloads, that will be divided in chunks using fragmented
    // messages, and Continue opcode
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
