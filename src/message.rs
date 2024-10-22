use crate::error::Error;
use crate::frame::{Frame, OpCode};

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
}

impl Message {
    // Converts a Frame into a Message variant
    pub fn from_frame(frame: Frame) -> Result<Self, Error> {
        match frame.opcode {
            OpCode::Text => Ok(Message::Text(String::from_utf8(frame.payload)?)),
            OpCode::Binary => Ok(Message::Binary(frame.payload)),
            _ => Err(Error::InvalidOpcode),
        }
    }

    // Function to get the payload as binary (Vec<u8>)
    pub fn as_binary(&self) -> Vec<u8> {
        match self {
            Message::Text(text) => text.as_bytes().to_vec(),
            Message::Binary(data) => data.clone(),
        }
    }

    // Function to get the payload as a String
    pub fn as_text(&self) -> Result<String, Error> {
        match self {
            Message::Text(text) => Ok(text.clone()),
            Message::Binary(data) => Ok(String::from_utf8(data.clone())?),
        }
    }

    // Function to convert Message to Frames
    pub fn to_frames(self, max_frame_size: usize) -> Vec<Frame> {
        let opcode = match self {
            Message::Text(_) => OpCode::Text,
            Message::Binary(_) => OpCode::Binary,
        };

        let payload = match self {
            Message::Text(text) => text.into_bytes(),
            Message::Binary(data) => data,
        };

        if payload.is_empty() {
            return vec![Frame {
                final_fragment: true,
                opcode,
                payload,
            }];
        }

        let mut frames = Vec::new();
        for chunk in payload.chunks(max_frame_size) {
            frames.push(Frame {
                final_fragment: false,
                opcode: if frames.is_empty() {
                    opcode.clone()
                } else {
                    OpCode::Continue
                },
                payload: chunk.to_vec(),
            });
        }

        if let Some(last_frame) = frames.last_mut() {
            last_frame.final_fragment = true;
        }

        frames
    }
}
