use std::io;
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
}

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("{source}")]
    IOError {
        #[from]
        source: io::Error,
    },

    #[error("test")]
    TestError,

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