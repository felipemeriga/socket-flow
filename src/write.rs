use std::io;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{Receiver};
use crate::error::StreamError;
use crate::frame::{Frame, OpCode};

pub struct WriteStream<W: AsyncWriteExt + Unpin> {
    pub write: W,
    broadcast_rx: Receiver<Frame>,
    internal_rx: Receiver<Frame>,
}

impl<W: AsyncWriteExt + Unpin> WriteStream<W> {
    pub fn new(write: W, broadcast_rx: Receiver<Frame>, internal_rx: Receiver<Frame>) -> Self {
        Self { write, broadcast_rx, internal_rx }
    }

    pub async fn run(&mut self) -> Result<(), StreamError> {
        loop {
            tokio::select! {
                broadcast_data = self.broadcast_rx.recv() => {
                    match broadcast_data {
                        Some(data) => {
                            let data_clone = data.clone();
                            self.write_frame(data).await?;
                            if data_clone.opcode == OpCode::Close {
                                 break;
                            }
                        },
                        None => break
                    }
                },
                internal_data = self.internal_rx.recv() => {
                    match internal_data {
                        Some(frame) => {
                            self.write_frame(frame).await?
                        },
                        None => break
                    }
                },
            }
        }
        Ok(())
    }

    pub async fn write_frame(&mut self, frame: Frame) -> io::Result<()> {
        // The first byte of a websockets frame contains the final fragment bit, and the OpCode
        // in (frame.final_fragment as u8) << 7 we are doing a left bitwise shift, if final_fragment is true
        // it will be converted from 10000000 to 1
        // after that it will perform a bitwise OR operation with OpCode, so if Opcode is text(0x1)
        // the final result will be 10000001, which is 129 decimal
        let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
        let payload_len = frame.payload.len();

        self.write.write_all(&[first_byte]).await?;

        // According to Websockets RFC, if the payload length is less or equal 125, it's written as a 8-bit unsigned integer
        // if it's between 126 and 65535, it's represented by additional 8 bytes.
        if payload_len <= 125 {
            self.write.write_all(&[payload_len as u8]).await?;
        } else if payload_len <= 65535 {
            self.write.write_all(&[126, (payload_len >> 8) as u8, payload_len as u8]).await?;
        } else {
            let bytes = payload_len.to_be_bytes();
            self.write.write_all(&[127, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]).await?;
        }

        self.write.write_all(&frame.payload).await?;

        Ok(())
    }
}