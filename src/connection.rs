use crate::error::Error;
use crate::message::Message;
use crate::split::{WSReader, WSWriter};
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

// const CLOSE_TIMEOUT: u64 = 5;
// TODO - Instead of using writer and read_rx, use directly WSRead and WSWrite,
// and return  the struct own attributes when splitting it
pub struct WSConnection {
    writer: WSWriter,
    reader: WSReader
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
        Pin::new(&mut this.reader).poll_next(cx)
    }
}

impl WSConnection {
    pub fn new(
        writer: WSWriter,
        reader: WSReader,
    ) -> Self {
        Self {
            writer,
            reader
        }
    }

    /// This function will split the connection into the `WSReader`, which is a stream of messages
    /// and `WSWriter`, for writing data into the socket.
    /// It's a good option when you need to work with both in separate tasks or functions
    pub fn split(self) -> (WSReader, WSWriter) {
        (self.reader, self.writer)
    }

    /// This function will be used for closing the connection between two instances, mainly it will
    /// be used by a client,
    /// to request disconnection with a server.It first sends a close frame
    /// through the socket, and waits until it receives the confirmation in a channel
    /// executing it inside a timeout, to avoid a long waiting time
    pub async fn close_connection(&mut self) -> Result<(), Error> {
        self.writer.close_connection().await
    }

    pub async fn send_message(&mut self, message: Message) -> Result<(), Error> {
        self.writer.send_message(message).await
    }

    // This function will be used to send general data as a Vector of bytes, and by default will
    // be sent as a text opcode
    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.writer.send(data).await
    }

    pub async fn send_as_binary(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.writer.send_as_binary(data).await
    }

    pub async fn send_as_text(&mut self, data: String) -> Result<(), Error> {
        self.writer.send_as_text(data).await
    }


    // It will send a ping frame through the socket
    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.writer.send_ping().await
    }

    // This function can be used to send large payloads, that will be divided in chunks using fragmented
    // messages, and Continue opcode
    pub async fn send_large_data_fragmented(&mut self, data: Vec<u8>, fragment_size: usize) -> Result<(), Error> {
        self.writer.send_large_data_fragmented(data, fragment_size).await
    }
}
