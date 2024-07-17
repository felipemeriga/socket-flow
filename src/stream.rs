use std::io;
use std::io::{Error, ErrorKind};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::mpsc::error::SendError;
use tokio::time::{timeout, Duration};
use crate::frame::{Frame, MAX_PAYLOAD_SIZE, OpCode};

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("{source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("{source}")]
    SendError {
        #[from]
        source: SendError<Vec<u8>>,
    },
}

pub struct Stream<R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin> {
    pub read: R,
    pub write: W,
    fragmented_message: Option<Vec<u8>>,
    read_tx: UnboundedSender<Vec<u8>>,
    write_rx: UnboundedReceiver<Vec<u8>>,
}

impl<R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin> Stream<R, W> {
    pub fn new(read: R, write: W, read_tx: UnboundedSender<Vec<u8>>, write_rx: UnboundedReceiver<Vec<u8>>) -> Self {
        let fragmented_message = Some(Vec::new());
        Self { read, write, fragmented_message, read_tx, write_rx }
    }

    // TODO: Add more descriptive errors for each Opcode handling, using thiserror
    // of the own package printing the error like:  eprintln!("Error while reading frame: {}", e);
    pub async fn poll_messages(&mut self) -> Result<(), StreamError> {
        // Now in websocket mode, read frames
        loop {
            match self.read_frame().await {
                Ok(frame) => {
                    match frame.opcode {
                        OpCode::Continue => {
                            // Check to see if there is an existing-fragmented message
                            // Append the payload to the existing one
                            if let Some(ref mut fragmented_message) = self.fragmented_message {
                                fragmented_message.extend_from_slice(&frame.payload);

                                // If it's the final fragment, then you can process the complete message here.
                                // You could move the message to somewhere else as well.
                                if frame.final_fragment {
                                    println!("Received fragmented message with total length: {}", fragmented_message.len());
                                    // Clean the buffer after processing
                                    self.fragmented_message = None;
                                }
                            } else {
                                eprintln!("Invalid continuation frame: no fragmented message to continue");
                                break;
                            }
                        }
                        OpCode::Text => {
                            self.read_tx.send(frame.payload)?
                        }
                        OpCode::Binary => {
                            // Handle Binary data here. For example, let's just print the length of the data.
                            println!("Received binary data of length: {}", frame.payload.len());
                        }
                        OpCode::Close => {
                            if let Err(e) = self.send_close_frame().await {
                                eprintln!("Failed to send Close Frame: {}", e);
                            }
                            break;
                        }
                        OpCode::Ping => {
                            if let Err(e) = self.send_pong_frame(frame.payload).await {
                                eprintln!("Failed to send Pong Frame: {}", e);
                            }
                        }
                        OpCode::Pong => {
                            // handle Pong here or just absorb and do nothing
                            // You could implement code to log these messages or perform other custom behavior
                        }
                    }
                }
                Err(error) => Err(error)?
            }
        }
        Ok(())
    }

    async fn send_pong_frame(&mut self, payload: Vec<u8>) -> io::Result<()> {
        let pong_frame = Frame::new(true, OpCode::Pong, payload);
        self.write_frame(pong_frame).await
    }

    pub async fn read_frame(&mut self) -> Result<Frame, Error> {
        let mut header = [0u8; 2];

        self.read.read_exact(&mut header).await?;

        // The first bit in the first byte in the frame tells us whether the current frame is the final fragment of a message
        let final_fragment = (header[0] & 0b10000000) != 0;
        // The opcode is the last 4 bits of the first byte in a websockets frame, here we are doing a bitwise AND operation & 0b00001111
        // to get the last 4 bits of the first byte
        let opcode = OpCode::from(header[0] & 0b00001111);

        // As a rule in websockets protocol, if your opcode is a control opcode(ping,pong,close), your message can't be fragmented(split between multiple frames)
        if !final_fragment && opcode.is_control() {
            return Err(Error::new(ErrorKind::InvalidInput, "Control frames must not be fragmented"));
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
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Payload too large",
            ));
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
            Ok(Ok(_)) => {} // Continue processing the payload
            Ok(Err(e)) => return Err(e), // An error occurred while reading
            Err(_e) => return Err(Error::new(io::ErrorKind::TimedOut, "Timed out reading from socket")), // Reading from the socket timed out
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

    pub async fn write_frame(&mut self, frame: Frame) -> io::Result<()> {
        let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
        let initial_payload_len = frame.payload.len() as u8;

        self.write.write_all(&[first_byte, initial_payload_len]).await?;
        self.write.write_all(&frame.payload).await?;

        Ok(())
    }

    pub async fn send_close_frame(&mut self) -> io::Result<()> {
        self.write_frame(Frame::new(true, OpCode::Close, Vec::new())).await
    }
}

// The Stream contains the split socket and unbounded channels, since Stream is the only one that
// holds the ownership to BufReader, WriteHalf, read_tx and write_tx. If the created struct goes out
// of scopes, it will be dropped automatically, also the another dependencies to the unbounded channels
// will be closed.
impl<R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin> Drop for Stream<R, W> {
    fn drop(&mut self) {
        // No need to manually drop parts of our struct, Rust will take care of it automatically.
    }
}