use crate::frame::Frame;
use std::io;
use std::string::FromUtf8Error;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::time::error::Elapsed;

#[derive(Error, Debug)]
pub enum CloseError {
    #[error("{source}")]
    SendError {
        #[from]
        source: SendError<Frame>,
    },

    #[error("{source}")]
    Timeout {
        #[from]
        source: Elapsed,
    },
}

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
        source: FromUtf8Error,
    },

    #[error("Server didn't upgrade the connection")]
    NoUpgrade,

    #[error("Sever didn't send a valid Sec-WebSocket-Accept key")]
    InvalidAcceptKey,
}

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("{source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("channel communication error")]
    CommunicationError,

    #[error("{source}")]
    InternalSendError {
        #[from]
        source: SendError<Frame>,
    },

    #[error("{source}")]
    CloseChannelError {
        #[from]
        source: SendError<bool>,
    },

    #[error("Incoming fragmented message but there is one already in progress")]
    FragmentedInProgress,

    #[error("Invalid continuation frame: no fragmented message to continue")]
    InvalidContinuationFrame,
}
