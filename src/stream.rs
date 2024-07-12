use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::frame::{OpCode, read_frame, send_close_frame};

pub struct WebsocketsStream<R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin> {
    pub read: R,
    pub write: W
}

impl <R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin>WebsocketsStream<R, W> {
    pub async fn poll_messages(&mut self) {
        // Now in websocket mode, read frames
        loop {
            match read_frame(&mut self.read).await {
                Ok(frame) => {
                    match frame.opcode {
                        OpCode::Continue => {}
                        OpCode::Text => {
                            let result = String::from_utf8(frame.payload);
                            match result {
                                Ok(v) => println!("{}", v),
                                Err(e) => println!("Invalid UTF-8 sequence: {}", e),
                            };
                        }
                        OpCode::Binary => {}
                        OpCode::Close => {
                            if let Err(e) = send_close_frame(&mut self.write).await {
                                eprintln!("Failed to send Close Frame: {}", e);
                            }
                            break;
                        }
                        OpCode::Ping => {}
                        OpCode::Pong => {}
                    }
                }
                Err(e) => {
                    eprintln!("Error while reading frame: {}", e);
                    break;
                }
            }
        }
    }


}