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

    pub async fn run(&mut self) -> Result<(), StreamError>{
        loop {
            tokio::select! {
                Some(data) = self.broadcast_rx.recv() => {
                    let data_clone = data.clone();
                    self.write_frame(data).await?;
                    match data_clone.opcode {
                        OpCode::Close =>{
                            println!("Sending close frame");
                            break
                        },
                    _ => {}}
                }
                Some(frame) = self.internal_rx.recv() => {
                   self.write_frame(frame).await?
                }
                else => break,
            }
        }
        Ok(())
    }

    pub async fn write_frame(&mut self, frame: Frame) -> io::Result<()> {
        let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
        let initial_payload_len = frame.payload.len() as u8;

        self.write
            .write_all(&[first_byte, initial_payload_len])
            .await?;
        self.write.write_all(&frame.payload).await?;

        Ok(())
    }
}