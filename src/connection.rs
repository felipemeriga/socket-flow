use crate::error::Error;
use crate::frame::{Frame, OpCode, MAX_PAYLOAD_SIZE};
use futures::Stream;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

// const CLOSE_TIMEOUT: u64 = 5;
pub struct WSConnection {
    buf_reader: BufReader<ReadHalf<TcpStream>>,
    write_half: WriteHalf<TcpStream>,
    fragmented_message: Option<FragmentedMessage>,
}

#[derive(Clone)]
struct FragmentedMessage {
    fragments: Vec<u8>,
    op_code: OpCode,
}

impl Stream for WSConnection {
    type Item = Result<Frame, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // Polling the read_frame method to get the next frame
        match futures::executor::block_on(this.read_frame()) {
            Ok(frame) => {
                match frame.opcode {
                    OpCode::Text | OpCode::Binary if !frame.final_fragment => {
                        if this.fragmented_message.is_none() {
                            this.fragmented_message = Some(FragmentedMessage {
                                op_code: frame.opcode,
                                fragments: frame.payload,
                            });
                        } else {
                            return Poll::Ready(Some(Err(Error::FragmentedInProgress)));
                        }
                    }
                    OpCode::Continue => {
                        if let Some(ref mut fragmented_message) = this.fragmented_message {
                            fragmented_message
                                .fragments
                                .extend_from_slice(&frame.payload);

                            if frame.final_fragment {
                                let fragmented_message_clone = fragmented_message.clone();
                                this.fragmented_message = None;

                                return Poll::Ready(Some(Ok(Frame::new(
                                    true,
                                    fragmented_message_clone.op_code,
                                    fragmented_message_clone.fragments,
                                ))));
                            }
                        } else {
                            return Poll::Ready(Some(Err(Error::InvalidContinuationFrame)));
                        }
                    }
                    OpCode::Text | OpCode::Binary => {
                        if this.fragmented_message.is_some() {
                            return Poll::Ready(Some(Err(Error::InvalidFrameFragmentation)));
                        }

                        return Poll::Ready(Some(Ok(frame)));
                    }
                    OpCode::Close => {
                        let _ = futures::executor::block_on(this.send_close_frame());
                        return Poll::Ready(None);
                    }
                    OpCode::Ping => {
                        let _ = futures::executor::block_on(this.send_pong_frame(frame.payload));
                    }
                    OpCode::Pong => {
                        // handle Pong or ignore
                    }
                }
            }
            Err(e) => return Poll::Ready(Some(Err(e))),
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl WSConnection {
    pub fn new(read: BufReader<ReadHalf<TcpStream>>, write_half: WriteHalf<TcpStream>) -> Self {
        let fragmented_message = None;
        Self {
            buf_reader: read,
            write_half,
            fragmented_message,
        }
    }

    // This function will be used for closing the connection between two instances, mainly it will
    // be used by a client,
    // to request disconnection with a server.It first sends a close frame
    // through the socket, and waits until it receives the confirmation in a channel
    // executing it inside a timeout, to avoid a long waiting time
    pub async fn close_connection(&mut self) -> Result<(), io::Error> {
        self.write_frame(Frame::new(true, OpCode::Close, Vec::new()))
            .await?;

        sleep(Duration::from_secs(5)).await;

        Ok(())

        // match timeout(Duration::from_secs(CLOSE_TIMEOUT), self.write.lock().await.closed()).await {
        //     Err(err) => Err(err)?,
        //     _ => Ok(()),
        // }
    }

    // This function can be used to send any frame, with a specific payload through the socket
    pub async fn send_frame(&mut self, frame: Frame) -> std::io::Result<()> {
        self.write_frame(frame).await
    }

    // This function will be used to send general data as a Vector of bytes, and by default will
    // be sent as a text opcode
    pub async fn send_data(&mut self, data: Vec<u8>) -> std::io::Result<()> {
        self.write_frame(Frame::new(true, OpCode::Text, data)).await
    }

    // It will send a ping frame through the socket
    pub async fn send_ping(&mut self) -> std::io::Result<()> {
        self.write_frame(Frame::new(true, OpCode::Ping, Vec::new()))
            .await
    }

    // This function can be used to send large payloads, that will be divided in chunks using fragmented
    // messages, and Continue opcode
    pub async fn send_large_data_fragmented(&mut self, data: Vec<u8>) -> Result<(), io::Error> {
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

    async fn send_pong_frame(&mut self, payload: Vec<u8>) -> std::io::Result<()> {
        let pong_frame = Frame::new(true, OpCode::Pong, payload);
        self.write_frame(pong_frame).await
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
        // needs to fail, immediately
        let rsv1 = (header[0] & 0b01000000) != 0;
        let rsv2 = (header[0] & 0b00100000) != 0;
        let rsv3 = (header[0] & 0b00010000) != 0;

        if rsv1 || rsv2 || rsv3 {
            return Err(Error::RSVNotZero);
        }

        // As a rule in websockets protocol, if your opcode is a control opcode(ping,pong,close), your message can't be fragmented(split between multiple frames)
        if !final_fragment && opcode.is_control() {
            Err(Error::ControlFramesFragmented)?;
        }

        // According to the websocket protocol specification, the first bit of the second byte of each frame is the "Mask bit"
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

        if length > MAX_PAYLOAD_SIZE {
            Err(Error::PayloadSize)?;
        }

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
        // TODO - Need to verify if this is going to work with Continue Opcodes, and with valid big payloads, also with valid connections
        // that has a slow network
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

    pub async fn send_close_frame(&mut self) -> std::io::Result<()> {
        self.write_frame(Frame::new(true, OpCode::Close, Vec::new()))
            .await
    }

    pub async fn write_frame(&mut self, frame: Frame) -> io::Result<()> {
        // The first byte of a websockets frame contains the final fragment bit, and the OpCode
        // in (frame.final_fragment as u8) << 7 we are doing a left bitwise shift, if final_fragment is true
        // it will be converted from 10000000 to 1
        // after that it will perform a bitwise OR operation with OpCode, so if Opcode is text(0x1)
        // the final result will be 10000001, which is 129 decimal
        let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
        let payload_len = frame.payload.len();

        self.write_half.write_all(&[first_byte]).await?;

        // According to Websockets RFC, if the payload length is less or equal 125, it's written as a 8-bit unsigned integer
        // if it's between 126 and 65535, it's represented by additional 8 bytes.
        if payload_len <= 125 {
            self.write_half.write_all(&[payload_len as u8]).await?;
        } else if payload_len <= 65535 {
            self.write_half
                .write_all(&[126, (payload_len >> 8) as u8, payload_len as u8])
                .await?;
        } else {
            let bytes = payload_len.to_be_bytes();
            self.write_half
                .write_all(&[
                    127, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                    bytes[7],
                ])
                .await?;
        }

        self.write_half.write_all(&frame.payload).await?;

        Ok(())
    }
}
