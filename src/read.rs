use crate::config::WebSocketConfig;
use crate::error::Error;
use crate::frame::{Frame, OpCode};
use crate::message::Message;
use crate::stream::SocketFlowStream;
use crate::write::Writer;
use std::sync::Arc;
use bytes::BytesMut;
use sha1::digest::typenum::op;
use tokio::io::{AsyncReadExt, BufReader, ReadHalf};
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use crate::compression::Decoder;

#[derive(Clone)]
pub(crate) struct FragmentedMessage {
    fragments: Vec<u8>,
    op_code: OpCode,
    compressed: bool,
}

pub struct ReadStream {
    buf_reader: BufReader<ReadHalf<SocketFlowStream>>,
    fragmented_message: Option<FragmentedMessage>,
    pub read_tx: Sender<Result<Message, Error>>,
    writer: Arc<Mutex<Writer>>,
    config: WebSocketConfig,
    decoder: Decoder,
}

impl ReadStream {
    pub fn new(
        read: BufReader<ReadHalf<SocketFlowStream>>,
        read_tx: Sender<Result<Message, Error>>,
        writer: Arc<Mutex<Writer>>,
        config: WebSocketConfig,
        decoder: Decoder,
    ) -> Self {
        let fragmented_message = None;
        Self {
            buf_reader: read,
            fragmented_message,
            read_tx,
            writer,
            config,
            decoder,
        }
    }

    // Compression plan for read.rs
    // When rsv1 = 1, and compression is enabled, I will continue, otherwise will disconnect
    // If FIN = 1, and it's compressed, I will unmask it and uncompress directly in read_frame function
    // If FIN = 0, I will move the first match case of poll_messages to read_frame, start a new fragmented message
    // and set a new attribute on the fragmented message, telling that it's fragmented
    // when I receive the last fragment, I will uncompress the the entire payload: Vec<u8>
    pub async fn poll_messages(&mut self) -> Result<(), Error> {
        // Now in websocket mode, read frames
        loop {
            match self.read_frame().await {
                Ok(frame) => {
                    match frame.opcode {
                        // By default, in order to start a fragmented message, the first frame should have a Text or Binary opcode,
                        // with a FIN bit set to 0
                        OpCode::Text | OpCode::Binary if !frame.final_fragment => {
                            // Starting a new fragmented message
                            if self.fragmented_message.is_none() {
                                self.fragmented_message = Some(FragmentedMessage {
                                    op_code: frame.opcode,
                                    fragments: frame.payload,
                                    compressed: frame.compressed,
                                });
                            } else {
                                Err(Error::FragmentedInProgress)?
                            }
                        }
                        // Per WebSockets RFC, the Continue opcode is specifically meant for continuation frames of a fragmented message
                        // The first frame of a fragmented message should contain either a text(0x1) or binary(0x2) opcode.
                        // From the second frame to the last frame but one, the opcode should be set to continue (0x0),
                        // and the fin set to 0. The last frame should have the opcode set to continue and fin set to 1
                        OpCode::Continue => {
                            if let Some(ref mut fragmented_message) = self.fragmented_message {
                                fragmented_message
                                    .fragments
                                    .extend_from_slice(&frame.payload);

                                if fragmented_message.fragments.len()
                                    > self.config.max_message_size.unwrap_or_default()
                                {
                                    Err(Error::MaxMessageSize)?;
                                }

                                let mut fragmented_message_clone = fragmented_message.clone();
                                // If it's the final fragment, then you can process the complete message here.
                                // You could move the message to somewhere else as well.
                                if frame.final_fragment {
                                    // Clean the buffer after processing
                                    self.fragmented_message = None;
                                    if fragmented_message_clone.compressed {
                                        fragmented_message_clone.fragments = self.decoder.decompress(&mut fragmented_message_clone.fragments)?;
                                    }

                                    // TODO - Decompression if compression is enabled
                                    // Since a clone copies the entire reference to a new reference,
                                    // if you change the original data,
                                    // the copy won't be modified
                                    // and this copy variable will be dropped
                                    // when the scope of this
                                    // match ends
                                    self.transmit_message(Frame::new(
                                        true,
                                        fragmented_message_clone.op_code,
                                        fragmented_message_clone.fragments,
                                        false,
                                    ))
                                        .await?;
                                }
                            } else {
                                Err(Error::InvalidContinuationFrame)?
                            }
                        }
                        OpCode::Text | OpCode::Binary => {
                            // If we have a fragmented message in progress, and we receive a Text or Binary
                            // with FIN bit as 1(final), before receiving a Continue Opcode with FIN bit 1(Last fragment)
                            // we should disconnect
                            if self.fragmented_message.is_some() {
                                Err(Error::InvalidFrameFragmentation)?
                            }

                            self.transmit_message(frame).await?;
                        }
                        OpCode::Close => {
                            // Either if this is being used as a client or server, per websocket
                            // RFC, if we receive a close,
                            // we need to respond with a close opcode.
                            // If the close was initiated by this library, we still want to call
                            // send_close_frame, to close all the tokio mpsc channels of this connection

                            self.send_close_frame().await?;

                            break;
                        }
                        OpCode::Ping => {
                            self.send_pong_frame(frame.payload).await?;
                        }
                        OpCode::Pong => {
                            // handle Pong here or just absorb and do nothing
                            // You could implement code to log these messages or perform other custom behavior
                        }
                    }
                }
                Err(error) => Err(error)?,
            }
        }
        Ok(())
    }

    async fn send_pong_frame(&mut self, payload: Vec<u8>) -> Result<(), Error> {
        let pong_frame = Frame::new(true, OpCode::Pong, payload, false);
        self.writer.lock().await.write_frame(pong_frame).await
    }

    pub async fn read_frame(&mut self) -> Result<Frame, Error> {
        let mut header = [0u8; 2];

        self.buf_reader.read_exact(&mut header).await?;

        // The first bit in the first byte in the frame tells us whether the current frame is the final fragment of a message
        // here we are getting the native binary 0b10000000 and doing a bitwise AND operation
        let final_fragment = (header[0] & 0b10000000) != 0;
        // The opcode is the last 4 bits of the first byte in a websockets frame, here we are doing a bitwise AND operation & 0b00001111
        // to get the last 4 bits of the first byte
        let opcode = OpCode::from(header[0] & 0b00001111)?;

        // RSV is a short for "Reserved" fields, they are optional flags that aren't used by the
        // base websockets protocol, only if there is an extension of the protocol in use.
        // If these bits are received as non-zero in the absence of any defined extension, the connection
        // needs to fail immediately
        let rsv1 = (header[0] & 0b01000000) != 0;
        let rsv2 = (header[0] & 0b00100000) != 0;
        let rsv3 = (header[0] & 0b00010000) != 0;

        if rsv2 || rsv3 || (rsv1 && !self.config.extensions.clone().unwrap_or_default().permessage_deflate) {
            return Err(Error::RSVNotZero);
        }

        // As a rule in websockets protocol,
        // if your opcode is a control opcode(ping,pong,close), your message can't be fragmented
        // (split between multiple frames)
        if !final_fragment && opcode.is_control() {
            Err(Error::ControlFramesFragmented)?;
        }

        // According to the websocket protocol specification,
        // the first bit of the second byte of each frame is the "Mask bit,"
        // it tells us if the payload is masked or not
        let masked = (header[1] & 0b10000000) != 0;

        // In the second byte of a WebSocket frame, the first bit is used to represent the
        // Mask bit - which we discussed before - and the next 7 bits are used to represent the
        // payload length, or the size of the data being sent in the frame.
        let mut length = (header[1] & 0b01111111) as usize;

        // Control frames are only allowed to have a payload up to and including 125 octets
        if length > 125 && opcode.is_control() {
            Err(Error::ControlFramePayloadSize)?;
        }

        if length == 126 {
            let mut be_bytes = [0u8; 2];
            self.buf_reader.read_exact(&mut be_bytes).await?;
            length = u16::from_be_bytes(be_bytes) as usize;
        } else if length == 127 {
            let mut be_bytes = [0u8; 8];
            self.buf_reader.read_exact(&mut be_bytes).await?;
            length = u64::from_be_bytes(be_bytes) as usize;
        }

        if length > self.config.max_frame_size.unwrap_or_default() {
            Err(Error::MaxFrameSize)?;
        }

        // According to Websockets RFC, a client should always send masked frames,
        // while frames sent from server to a client are not masked
        let mask = if masked {
            let mut mask = [0u8; 4];
            self.buf_reader.read_exact(&mut mask).await?;
            Some(mask)
        } else {
            None
        };

        let mut payload = vec![0u8; length];

        // Adding a timeout function from Tokio, to avoid malicious TCP connections, that passes through handshake
        // and starts to send invalid websockets frames to overload the socket
        // Since HTTP is an application protocol built on the top of TCP, a malicious TCP connection may send a string with the HTTP content in the
        // first connection, to simulate a handshake, and start sending huge payloads.
        let read_result = timeout(
            Duration::from_secs(5),
            self.buf_reader.read_exact(&mut payload),
        )
            .await;
        match read_result {
            Ok(Ok(_)) => {}        // Continue processing the payload
            Ok(Err(e)) => Err(e)?, // An error occurred while reading
            Err(_e) => Err(_e)?,   // Reading from the socket timed out
        }

        // Unmasking,
        // According to the WebSocket protocol, all frames sent from the client to the server must be
        // masked by a four-byte value, which is often random. This "masking key" is part of the frame
        // along with the payload data and helps to prevent specific bytes from being discernible on the
        // network.
        // The mask is applied using a simple bitwise XOR operation. Each byte of the payload data
        // is XOR'd with the corresponding byte (modulo 4) of the 4-byte mask. The server then uses
        // the masking key to reverse the process, recovering the original data.
        if let Some(mask) = mask {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
        }


        if rsv1 && final_fragment {
            payload = self.decoder.decompress(&payload)?; // Call your custom decompression function
        }

        Ok(Frame {
            final_fragment,
            opcode,
            payload,
            compressed: rsv1,
        })
    }

    pub async fn send_close_frame(&mut self) -> Result<(), Error> {
        self.writer
            .lock()
            .await
            .write_frame(Frame::new(true, OpCode::Close, Vec::new(), false))
            .await
    }

    pub async fn transmit_message(&mut self, frame: Frame) -> Result<(), Error> {
        // According to WebSockets RFC, The text opcode MUST be encoded as UTF-8
        if frame.opcode == OpCode::Text {
            let text_payload = frame.clone().payload;
            _ = String::from_utf8(text_payload)?
        }

        self.read_tx
            .send(Ok(Message::from_frame(frame)?))
            .await
            .map_err(|_| Error::CommunicationError)
    }
}

// Since ReadStream is the only one that
// holds the ownership to BufReader, and read_tx. If the created struct goes out
// of scopes, it will be dropped automatically, also another dependency to the channels
// will be closed.
// Therefore, if these attributes are dropped, all channels will be closed, and the TCP connection, terminated
impl Drop for ReadStream {
    fn drop(&mut self) {
        // No need to manually drop parts of our struct, Rust will take care of it automatically.
    }
}
