use crate::error::StreamError;
use crate::frame::{Frame, OpCode, MAX_PAYLOAD_SIZE};
use std::io;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

type ReadTransmitter = Arc<Mutex<Sender<Result<Vec<u8>, StreamError>>>>;
pub enum StreamKind {
    Client,
    Server,
}

pub struct ReadStream<R: AsyncReadExt + Unpin> {
    kind: StreamKind,
    pub read: R,
    fragmented_message: Option<Vec<u8>>,
    read_tx: ReadTransmitter,
    internal_tx: Sender<Frame>,
    close_tx: Sender<bool>,
}

impl<R: AsyncReadExt + Unpin> ReadStream<R> {
    pub fn new(
        kind: StreamKind,
        read: R,
        read_tx: ReadTransmitter,
        internal_tx: Sender<Frame>,
        close_tx: Sender<bool>,
    ) -> Self {
        let fragmented_message = None;
        Self {
            kind,
            read,
            fragmented_message,
            read_tx,
            internal_tx,
            close_tx,
        }
    }

    //
    pub async fn poll_messages(&mut self) -> Result<(), StreamError> {
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
                                self.fragmented_message = Some(frame.payload);
                            } else {
                                Err(StreamError::FragmentedInProgress)?
                            }
                        }
                        // Per WebSockets RFC, the Continue opcode is specifically meant for continuation frames of a fragmented message
                        // The first frame of a fragmented message should contain either a text(0x1) or binary(0x2) opcode.
                        // From the second frame to the last frame but one, the opcode should be set to continue (0x0)
                        // and the fin set to 0. The last frame should have the opcode set to continue and fin set to 1
                        OpCode::Continue => {
                            if let Some(ref mut fragmented_message) = self.fragmented_message {
                                fragmented_message.extend_from_slice(&frame.payload);

                                let fragmented_message_clone = fragmented_message.clone();
                                // If it's the final fragment, then you can process the complete message here.
                                // You could move the message to somewhere else as well.
                                if frame.final_fragment {
                                    self.read_tx
                                        .lock()
                                        .await
                                        .send(Ok(fragmented_message_clone))
                                        .await
                                        .map_err(|_| StreamError::CommunicationError)?;

                                    // Clean the buffer after processing
                                    self.fragmented_message = None;
                                }
                            } else {
                                Err(StreamError::InvalidContinuationFrame)?
                            }
                        }
                        OpCode::Text | OpCode::Binary => {
                            self.read_tx
                                .lock()
                                .await
                                .send(Ok(frame.payload))
                                .await
                                .map_err(|_| StreamError::CommunicationError)?;
                        }
                        OpCode::Close => {
                            // We could use macros for implementing different behaviors for stream kinds
                            // but this match should solve this, avoid the code complexity, and the extra amount of
                            // code for a macro
                            match self.kind {
                                StreamKind::Server => {
                                    self.send_close_frame().await?;
                                    break;
                                }
                                StreamKind::Client => {
                                    self.close_tx.send(true).await?;
                                    break;
                                }
                            }
                        }
                        OpCode::Ping => self.send_pong_frame(frame.payload).await?,
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

    async fn send_pong_frame(&mut self, payload: Vec<u8>) -> Result<(), SendError<Frame>> {
        let pong_frame = Frame::new(true, OpCode::Pong, payload);
        self.internal_tx.send(pong_frame).await
    }

    pub async fn read_frame(&mut self) -> Result<Frame, Error> {
        let mut header = [0u8; 2];

        self.read.read_exact(&mut header).await?;

        // The first bit in the first byte in the frame tells us whether the current frame is the final fragment of a message
        // here we are getting the native binary 0b10000000 and doing a bitwise AND operation
        let final_fragment = (header[0] & 0b10000000) != 0;
        // The opcode is the last 4 bits of the first byte in a websockets frame, here we are doing a bitwise AND operation & 0b00001111
        // to get the last 4 bits of the first byte
        let opcode = OpCode::from(header[0] & 0b00001111)?;

        // As a rule in websockets protocol, if your opcode is a control opcode(ping,pong,close), your message can't be fragmented(split between multiple frames)
        if !final_fragment && opcode.is_control() {
            Err(Error::new(
                ErrorKind::InvalidInput,
                "Control frames must not be fragmented",
            ))?;
        }

        // According to the websocket protocol specification, the first bit of the second byte of each frame is the "Mask bit"
        // it tells us if the payload is masked or not
        let masked = (header[1] & 0b10000000) != 0;

        // In the second byte of a WebSocket frame, the first bit is used to represent the
        // Mask bit - which we discussed before - and the next 7 bits are used to represent the
        // payload length, or the size of the data being sent in the frame.
        let mut length = (header[1] & 0b01111111) as usize;

        if length == 126 {
            let mut be_bytes = [0u8; 2];
            self.read.read_exact(&mut be_bytes).await?;
            length = u16::from_be_bytes(be_bytes) as usize;
        } else if length == 127 {
            let mut be_bytes = [0u8; 8];
            self.read.read_exact(&mut be_bytes).await?;
            length = u64::from_be_bytes(be_bytes) as usize;
        }

        if length > MAX_PAYLOAD_SIZE {
            Err(Error::new(ErrorKind::InvalidData, "Payload too large"))?;
        }

        let mask = if masked {
            let mut mask = [0u8; 4];
            self.read.read_exact(&mut mask).await?;
            Some(mask)
        } else {
            None
        };

        let mut payload = vec![0u8; length];

        // Adding a timeout function from Tokio, to avoid malicious TCP connections, that passes through handshake
        // and starts to send invalid websockets frames to overload the socket
        // Since HTTP is an application protocol built on the top of TCP, a malicious TCP connection may send a string with the HTTP content in the
        // first connection, to simulate a handshake, and start sending huge payloads.
        // TODO - Need to verify if this is going to work with Continue Opcodes, and with valid big payloads, also with valid connections
        // that has a slow network
        let read_result = timeout(Duration::from_secs(5), self.read.read_exact(&mut payload)).await;
        match read_result {
            Ok(Ok(_)) => {}        // Continue processing the payload
            Ok(Err(e)) => Err(e)?, // An error occurred while reading
            Err(_e) => Err(Error::new(
                io::ErrorKind::TimedOut,
                "Timed out reading from socket",
            ))?, // Reading from the socket timed out
        }

        // Unmasking
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

        Ok(Frame {
            final_fragment,
            opcode,
            payload,
        })
    }

    pub async fn send_close_frame(&mut self) -> Result<(), SendError<Frame>> {
        self.internal_tx
            .send(Frame::new(true, OpCode::Close, Vec::new()))
            .await
    }
}

// The Stream contains the split socket and mpsc channels, since Stream is the only one that
// holds the ownership to BufReader, WriteHalf, read_tx and write_tx. If the created struct goes out
// of scopes, it will be dropped automatically, also the another dependencies to the channels
// will be closed.
// Therefore, if these attributes are dropped, and channels will be closed, and the TCP connection, terminated
impl<R: AsyncReadExt + Unpin> Drop for ReadStream<R> {
    fn drop(&mut self) {
        // No need to manually drop parts of our struct, Rust will take care of it automatically.
    }
}
