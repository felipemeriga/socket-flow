use crate::error::Error;
use crate::message::Message;
use crate::split::{WSReader, WSWriter};
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

/// WSConnection represents the final connection of a client/server, after all the steps
/// of establishing a connection have been properly met.
/// This structure will be delivered to the end-user, which contains the reader, which would be used
/// as a stream,
/// for reading all the data that comes in this connection, and the writer, for writing
/// data into the stream.
pub struct WSConnection {
    /// Represents the writer side of the connection, where the end-user can send data over the connection
    /// with a different set methods
    writer: WSWriter,
    /// Implements futures::Stream,
    /// so the end-user can process all the incoming messages, using .next() method
    reader: WSReader,
}

// WSConnection has the reader attribute, which is already a ReceiverStream
// Although, we don't want this attribute visible to the end-user.
// Therefore, implementing Stream for this struct is necessary, so end-user could
// invoke next() and other stream methods directly from a variable that holds this struct.
impl Stream for WSConnection {
    type Item = Result<Message, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // We need to get a mutable reference to the inner field
        let this = self.get_mut();

        // Delegate the polling to `reader`
        // We need to pin `reader` because its `poll_next` method requires the object to be pinned
        Pin::new(&mut this.reader).poll_next(cx)
    }
}

impl WSConnection {
    pub fn new(writer: WSWriter, reader: WSReader) -> Self {
        Self { writer, reader }
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

    /// Send a general message, which is a good option for echoing messages
    pub async fn send_message(&mut self, message: Message) -> Result<(), Error> {
        self.writer.send_message(message).await
    }

    /// Send generic data, by default it considers OpCode Text
    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.writer.send(data).await
    }

    /// Send a message as Binary Opcode
    pub async fn send_as_binary(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.writer.send_as_binary(data).await
    }

    /// Send a message as a String
    pub async fn send_as_text(&mut self, data: String) -> Result<(), Error> {
        self.writer.send_as_text(data).await
    }

    /// Sends a Ping OpCode to client/server
    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.writer.send_ping().await
    }

    /// Send data fragmented, where fragment_size should be a value calculated in powers of 2
    /// The payload would be divided into that size, still considering connection configurations
    /// like max_frame_size
    pub async fn send_large_data_fragmented(
        &mut self,
        data: Vec<u8>,
        fragment_size: usize,
    ) -> Result<(), Error> {
        self.writer
            .send_large_data_fragmented(data, fragment_size)
            .await
    }
}
