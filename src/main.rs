use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, split, ErrorKind};
use sha1::{Digest, Sha1};
use std::{io, str};
use std::io::{Error};
use std::net::Shutdown;
use bytes::BytesMut;
use tokio::time::{timeout, Duration};

#[tokio::main]
pub async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let (mut reader, mut writer) = split(socket);
            let mut buf_reader = BufReader::new(reader);

            let mut websocket_key = None;
            let mut buf = BytesMut::with_capacity(1024 * 16); // 16 kilobytes

            // Limit the maximum amount of data read to prevent a denial of service attack.
            while buf.len() <= 1024 * 16 {
                let mut tmp_buf = vec![0; 1024];
                match timeout(Duration::from_secs(10), buf_reader.read(&mut tmp_buf)).await {
                    Ok(Ok(0)) | Err(_) => break, // EOF reached or Timeout, we stop.
                    Ok(Ok(n)) => {
                        buf.extend_from_slice(&tmp_buf[..n]);
                        let s = String::from_utf8_lossy(&buf);
                        if let Some(start) = s.find("Sec-WebSocket-Key:") {
                            websocket_key = Some(s[start..].lines().next().unwrap().to_string());
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(key) = websocket_key {
                // Process the key line to extract the actual key value
                let key_value = parse_websocket_key(key);
                let accept_value = generate_websocket_accept_value(key_value.unwrap());

                let response = format!(
                    "HTTP/1.1 101 Switching Protocols\r\n\
        Connection: Upgrade\r\n\
        Upgrade: websocket\r\n\
        Sec-WebSocket-Accept: {}\r\n\
        \r\n", accept_value);
                writer.write_all(response.as_bytes()).await.unwrap();

                // Now in websocket mode, read frames
                loop {
                    match read_frame(&mut buf_reader).await {
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
                                    if let Err(e) = send_close_frame(&mut writer).await {
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
        });
    }
}

async fn write_frame<T: AsyncWriteExt + Unpin>(stream: &mut T, frame: Frame) -> io::Result<()> {
    let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
    let initial_payload_len = frame.payload.len() as u8;

    stream.write_all(&[first_byte, initial_payload_len]).await?;
    stream.write_all(&frame.payload).await?;

    Ok(())
}

async fn send_close_frame<T: AsyncWriteExt + Unpin>(stream: &mut T) -> io::Result<()> {
    // empty payload for the close frame
    let close_frame = Frame {
        final_fragment: true,
        opcode: OpCode::Close,
        payload: Vec::new(),
    };

    write_frame(stream, close_frame).await
}

#[derive(Debug)]
enum OpCode {
    Continue,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
    // other variants if needed...
}

impl OpCode {
    fn from(byte: u8) -> Self {
        match byte {
            0x0 => OpCode::Continue,
            0x1 => OpCode::Text,
            0x2 => OpCode::Binary,
            0x8 => OpCode::Close,
            0x9 => OpCode::Ping,
            0xA => OpCode::Pong,
            _ => panic!("Invalid opcode"), // handle unexpected opcode appropriately
        }
    }

    fn as_u8(&self) -> u8 {
        match self {
            OpCode::Continue => 0x0,
            OpCode::Text => 0x1,
            OpCode::Binary => 0x2,
            OpCode::Close => 0x8,
            OpCode::Ping => 0x9,
            OpCode::Pong => 0xA,
        }
    }

    fn is_control(&self) -> bool {
        matches!(self, OpCode::Close | OpCode::Ping | OpCode::Pong)
    }
}

#[derive(Debug)]
struct Frame {
    final_fragment: bool,
    opcode: OpCode,
    payload: Vec<u8>,
}
const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB

async fn read_frame<T: AsyncReadExt + Unpin>(buf_reader: &mut BufReader<T>) -> Result<Frame, Error> {
    let mut header = [0u8; 2];

    buf_reader.read_exact(&mut header).await?;

    let final_fragment = (header[0] & 0b10000000) != 0;
    let opcode = OpCode::from(header[0] & 0b00001111);

    if !final_fragment && opcode.is_control() {
        return Err(Error::new(ErrorKind::InvalidInput, "Control frames must not be fragmented"));
    }

    let masked = (header[1] & 0b10000000) != 0;

    let mut length = (header[1] & 0b01111111) as usize;

    if length == 126 {
        let mut be_bytes = [0u8; 2];
        buf_reader.read_exact(&mut be_bytes).await?;
        length = u16::from_be_bytes(be_bytes) as usize;
    } else if length == 127 {
        let mut be_bytes = [0u8; 8];
        buf_reader.read_exact(&mut be_bytes).await?;
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
        buf_reader.read_exact(&mut mask).await?;
        Some(mask)
    } else {
        None
    };

    let mut payload = vec![0u8; length];
    buf_reader.read_exact(&mut payload).await?;

    // Unmasking
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

fn parse_websocket_key(first_request: String) -> Option<String> {
    for line in first_request.lines() {
        if line.starts_with("Sec-WebSocket-Key: ") {
            return line[18..].split_whitespace().next().map(ToOwned::to_owned);
        }
    }
    None
}

fn generate_websocket_accept_value(key: String) -> String {
    let mut sha1 = Sha1::new();
    sha1.update(key.as_bytes());
    sha1.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());
    base64::encode(sha1.finalize())
}