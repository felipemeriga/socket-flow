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
}
