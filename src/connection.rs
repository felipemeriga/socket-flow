use crate::error::Error;
use crate::message::Message;
use crate::split::{WSReader, WSWriter};
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct WSConnection {
    writer: WSWriter,
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
    pub fn new(
        writer: WSWriter,
        reader: WSReader,
    ) -> Self {
        Self {
            writer,
            reader,
        }
    }

    /// This function will split the connection into the `WSReader`, which is a stream of messages
    /// and `WSWriter`, for writing data into the socket.
    /// It's a good option when you need to work with both in separate tasks or functions
    pub fn split(self) -> (WSReader, WSWriter) {
        (self.reader, self.writer)
    }

    pub async fn close_connection(&mut self) -> Result<(), Error> {
        self.writer.close_connection().await
    }

    pub async fn send_message(&mut self, message: Message) -> Result<(), Error> {
        self.writer.send_message(message).await
    }

    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.writer.send(data).await
    }

    pub async fn send_as_binary(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.writer.send_as_binary(data).await
    }

    pub async fn send_as_text(&mut self, data: String) -> Result<(), Error> {
        self.writer.send_as_text(data).await
    }

    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.writer.send_ping().await
    }

    pub async fn send_large_data_fragmented(&mut self, data: Vec<u8>, fragment_size: usize) -> Result<(), Error> {
        self.writer.send_large_data_fragmented(data, fragment_size).await
    }
}
