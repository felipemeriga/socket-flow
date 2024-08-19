use crate::error::Error;
use crate::frame::{Frame, OpCode};
use crate::write::Writer;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;

// const CLOSE_TIMEOUT: u64 = 5;
pub struct WSConnection {
    writer: Arc<Mutex<Writer>>,
    read_rx: ReceiverStream<Result<Frame, Error>>,
}

// WSConnection has the read_rx attribute, which is already a ReceiverStream
// Although, we don't want this attribute visible to the end-user.
// Therefore, implementing Stream for this struct is necessary, so end-user could
// invoke next() and other stream methods directly from a variable that holds this struct.
impl Stream for WSConnection {
    type Item = Result<Frame, Error>;

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
        read_rx: ReceiverStream<Result<Frame, Error>>,
    ) -> Self {
        Self {
            writer: write_half,
            read_rx,
        }
    }

    // TODO - Refactor how we are handling closes
    // This function will be used for closing the connection between two instances, mainly it will
    // be used by a client,
    // to request disconnection with a server.It first sends a close frame
    // through the socket, and waits until it receives the confirmation in a channel
    // executing it inside a timeout, to avoid a long waiting time
    pub async fn close_connection(&mut self) -> Result<(), Error> {
        self.write_frame(Frame::new(true, OpCode::Close, Vec::new()))
            .await?;

        sleep(Duration::from_millis(500)).await;

        Ok(())

        // match timeout(Duration::from_secs(CLOSE_TIMEOUT), self.write.lock().await.closed()).await {
        //     Err(err) => Err(err)?,
        //     _ => Ok(()),
        // }
    }

    // This function can be used to send any frame, with a specific payload through the socket
    pub async fn send_frame(&mut self, frame: Frame) -> Result<(), Error> {
        self.write_frame(frame).await
    }

    // This function will be used to send general data as a Vector of bytes, and by default will
    // be sent as a text opcode
    pub async fn send_data(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.write_frame(Frame::new(true, OpCode::Text, data)).await
    }

    // It will send a ping frame through the socket
    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.write_frame(Frame::new(true, OpCode::Ping, Vec::new()))
            .await
    }

    // This function can be used to send large payloads, that will be divided in chunks using fragmented
    // messages, and Continue opcode
    pub async fn send_large_data_fragmented(&mut self, data: Vec<u8>) -> Result<(), Error> {
        // We can set the MAX_FRAGMENT_SIZE to 65536 bytes(64KB), which is the maximum
        // size of a TCP packet. As TCP is based in packets, and HTTP and WS works on the top of TCP, any
        // fragment greater than 64KB would still work, since it will be divided into packets
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
                .await?
        }

        Ok(())
    }

    pub async fn write_frame(&mut self, frame: Frame) -> Result<(), Error> {
        self.writer.lock().await.write_frame(frame).await
    }
}
