use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
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
    pub fn from(byte: u8) -> Result<Self, Error> {
        match byte {
            0x0 => Ok(OpCode::Continue),
            0x1 => Ok(OpCode::Text),
            0x2 => Ok(OpCode::Binary),
            0x8 => Ok(OpCode::Close),
            0x9 => Ok(OpCode::Ping),
            0xA => Ok(OpCode::Pong),
            _ => Err(Error::InvalidOpcode),
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

#[derive(Debug, Clone)]
pub struct Frame {
    pub final_fragment: bool,
    pub opcode: OpCode,
    pub payload: Vec<u8>,
    pub compressed: bool,
}

impl Frame {
    pub fn new(final_fragment: bool, opcode: OpCode, payload: Vec<u8>, compressed: bool) -> Self {
        Self {
            final_fragment,
            opcode,
            payload,
            compressed,
        }
    }
}
