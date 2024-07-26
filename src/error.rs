use std::io;
use std::string::FromUtf8Error;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use crate::frame::Frame;

#[derive(Error, Debug)]
pub enum HandshakeError {
    #[error("Couldn't find Sec-WebSocket-Key header in the request")]
    NoSecWebsocketKey,

    #[error("IO Error happened: {source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("{source}")]
    FromUtf8Error {
        #[from]
        source: FromUtf8Error
    },

    #[error("Server didn't upgrade the connection")]
    NoUpgrade,

    #[error("Sever didn't send a valid Sec-WebSocket-Accept key")]
    InvalidAcceptKey
}

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("{source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("{source}")]
    BroadcastSendError {
        #[from]
        source: SendError<Vec<u8>>,
    },

    #[error("{source}")]
    InternalSendError {
        #[from]
        source: SendError<Frame>,
    },
}