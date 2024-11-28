use crate::config::WebSocketConfig;
use crate::encoder::Encoder;
use crate::error::Error;
use crate::frame::{Frame, OpCode};
use crate::message::Message;
use crate::write::Writer;
use bytes::BytesMut;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;

const PAYLOAD_SIZE_COMPRESSION_ENABLE: usize = 1;

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

pub struct WSWriter {
    pub writer: Arc<Mutex<Writer>>,
    pub web_socket_config: WebSocketConfig,
    encoder: Encoder,
}

impl WSWriter {
    pub fn new(
        writer: Arc<Mutex<Writer>>,
        web_socket_config: WebSocketConfig,
        encoder: Encoder,
    ) -> Self {
        Self {
            writer,
            web_socket_config,
            encoder,
        }
    }

    /// This function will be used for closing the connection between two instances, mainly it will
    /// be used by a client,
    /// to request disconnection with a server.It first sends a close frame
    /// through the socket, and waits until it receives the confirmation in a channel
    /// executing it inside a timeout, to avoid a long waiting time
    pub async fn close_connection(&mut self) -> Result<(), Error> {
        self.write_frames(vec![Frame::new(true, OpCode::Close, Vec::new(), false)])
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
        self.write_message(Message::Text(String::from_utf8(data)?))
            .await
    }

    pub async fn send_as_binary(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.write_message(Message::Binary(data)).await
    }

    pub async fn send_as_text(&mut self, data: String) -> Result<(), Error> {
        self.write_message(Message::Text(data)).await
    }

    // It will send a ping frame through the socket
    pub async fn send_ping(&mut self) -> Result<(), Error> {
        self.write_frames(vec![Frame::new(true, OpCode::Ping, Vec::new(), false)])
            .await
    }

    // This function can be used to send large payloads, that will be divided in chunks using fragmented
    // messages, and Continue opcode
    pub async fn send_large_data_fragmented(
        &mut self,
        mut data: Vec<u8>,
        fragment_size: usize,
    ) -> Result<(), Error> {
        // Each fragment size will be limited by max_frame_size config,
        // that had been given by the user,
        // or it will use the default max frame size which is 16 MiB.
        if fragment_size > self.web_socket_config.max_frame_size.unwrap_or_default() {
            return Err(Error::CustomFragmentSizeExceeded(
                fragment_size,
                self.web_socket_config.max_frame_size.unwrap_or_default(),
            ));
        }

        if data.len() > self.web_socket_config.max_message_size.unwrap_or_default() {
            return Err(Error::MaxMessageSize);
        }

        // This function will check if compression is enabled, and apply if needed
        let compressed = self.check_compression(&mut data)?;

        let chunks = data.chunks(fragment_size);
        let total_chunks = chunks.len();

        for (i, chunk) in chunks.enumerate() {
            let is_final = i == total_chunks - 1;
            let opcode = if i == 0 {
                OpCode::Text
            } else {
                OpCode::Continue
            };

            self.write_frames(vec![Frame::new(
                is_final,
                opcode,
                Vec::from(chunk),
                compressed,
            )])
            .await?
        }

        Ok(())
    }

    pub(crate) fn check_compression(&mut self, data: &mut Vec<u8>) -> Result<bool, Error> {
        let mut compressed = false;
        // If compression is enabled, and the payload is greater than 8KB, compress the payload
        if self
            .web_socket_config
            .extensions
            .clone()
            .unwrap_or_default()
            .permessage_deflate
            && data.len() > PAYLOAD_SIZE_COMPRESSION_ENABLE
        {
            *data = self.encoder.compress(&mut BytesMut::from(&data[..]))?;
            compressed = true;
        }

        Ok(compressed)
    }

    pub(crate) fn convert_to_frames(&mut self, message: Message) -> Result<Vec<Frame>, Error> {
        let opcode = match message {
            Message::Text(_) => OpCode::Text,
            Message::Binary(_) => OpCode::Binary,
        };

        let mut payload = match message {
            Message::Text(text) => text.into_bytes(),
            Message::Binary(data) => data,
        };

        // Empty payloads aren't compressed
        if payload.is_empty() {
            return Ok(vec![Frame {
                final_fragment: true,
                opcode,
                payload,
                compressed: false,
            }]);
        }

        let max_frame_size = self.web_socket_config.max_frame_size.unwrap_or_default();
        let mut frames = Vec::new();
        // This function will check if compression is enabled, and apply if needed
        let compressed = self.check_compression(&mut payload)?;

        for chunk in payload.chunks(max_frame_size) {
            frames.push(Frame {
                final_fragment: false,
                opcode: if frames.is_empty() {
                    opcode.clone()
                } else {
                    OpCode::Continue
                },
                payload: chunk.to_vec(),
                compressed,
            });
        }

        if let Some(last_frame) = frames.last_mut() {
            last_frame.final_fragment = true;
        }

        Ok(frames)
    }

    pub(crate) async fn write_message(&mut self, message: Message) -> Result<(), Error> {
        if message.as_binary().len() > self.web_socket_config.max_message_size.unwrap_or_default() {
            return Err(Error::MaxMessageSize);
        }

        let frames = self.convert_to_frames(message)?;
        self.write_frames(frames).await
    }

    pub(crate) async fn write_frames(&mut self, frames: Vec<Frame>) -> Result<(), Error> {
        // For compressed messages, regardless if it's fragmented or not, we always set the RSV1 bit
        // for the first frame.
        let mut set_rsv1_first_frame = !frames.is_empty() && frames[0].compressed;

        for frame in frames {
            self.writer
                .lock()
                .await
                .write_frame(frame, set_rsv1_first_frame)
                .await?;
            // Setting it to false,
            // since we only need
            // to set RSV1 bit for the first frame if compression is enabled
            set_rsv1_first_frame = false;
        }
        Ok(())
    }
}
