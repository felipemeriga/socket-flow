use std::io;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
pub enum OpCode {
    Continue,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
    // other variants if needed...
}

impl OpCode {
    pub fn from(byte: u8) -> Self {
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

    pub fn as_u8(&self) -> u8 {
        match self {
            OpCode::Continue => 0x0,
            OpCode::Text => 0x1,
            OpCode::Binary => 0x2,
            OpCode::Close => 0x8,
            OpCode::Ping => 0x9,
            OpCode::Pong => 0xA,
        }
    }

    pub fn is_control(&self) -> bool {
        matches!(self, OpCode::Close | OpCode::Ping | OpCode::Pong)
    }
}

#[derive(Debug)]
pub struct Frame {
    pub final_fragment: bool,
    pub opcode: OpCode,
    pub payload: Vec<u8>,
}
const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB


pub async fn read_frame<T: AsyncReadExt + Unpin>(buf_reader: &mut T) -> Result<Frame, Error> {
    let mut header = [0u8; 2];

    buf_reader.read_exact(&mut header).await?;

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

pub async fn write_frame<T: AsyncWriteExt + Unpin>(stream: &mut T, frame: Frame) -> io::Result<()> {
    let first_byte = (frame.final_fragment as u8) << 7 | frame.opcode.as_u8();
    let initial_payload_len = frame.payload.len() as u8;

    stream.write_all(&[first_byte, initial_payload_len]).await?;
    stream.write_all(&frame.payload).await?;

    Ok(())
}

pub async fn send_close_frame<T: AsyncWriteExt + Unpin>(stream: &mut T) -> io::Result<()> {
    // empty payload for the close frame
    let close_frame = Frame {
        final_fragment: true,
        opcode: OpCode::Close,
        payload: Vec::new(),
    };

    write_frame(stream, close_frame).await
}