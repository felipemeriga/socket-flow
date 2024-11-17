use crate::frame::Frame;
use httparse::Error as HttpParseError;
use pki_types::InvalidDnsNameError;
use std::io;
use std::string::FromUtf8Error;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::time::error::Elapsed;
use url::ParseError;

#[derive(Error, Debug)]
pub enum Error {
    // Sender / Receiver Errors
    #[error("{source}")]
    SendError {
        #[from]
        source: SendError<Frame>,
    },

    #[error("channel communication error")]
    CommunicationError,

    // General Errors
    #[error("{source}")]
    Timeout {
        #[from]
        source: Elapsed,
    },

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

    // Handshake Errors
    #[error("Invalid handshake request method and version")]
    InvalidHTTPHandshake,

    #[error("Connection: Upgrade header missing in the request")]
    NoConnectionHeaderPresent,

    #[error("Upgrade: websocket header missing in the request")]
    NoUpgradeHeaderPresent,

    #[error("Host header missing in the request")]
    NoHostHeaderPresent,

    #[error("Couldn't find Sec-WebSocket-Key header in the request")]
    NoSecWebsocketKey,

    #[error("Server didn't upgrade the connection")]
    NoUpgrade,

    #[error("Sever didn't send a valid Sec-WebSocket-Accept key")]
    InvalidAcceptKey,

    // Framing Errors
    #[error("RSV not zero")]
    RSVNotZero,

    #[error("Control frames must not be fragmented")]
    ControlFramesFragmented,

    #[error("Control frame with invalid payload size, can be greater than 125")]
    ControlFramePayloadSize,

    #[error("fragment_size: `{0}` can't be greater than max_frame_size: `{0}`")]
    CustomFragmentSizeExceeded(usize, usize),

    #[error("Max frame size reached")]
    MaxFrameSize,

    #[error("Max message size reached")]
    MaxMessageSize,

    // Fragmentation Errors
    #[error("Invalid frame while there is a fragmented message in progress")]
    InvalidFrameFragmentation,

    #[error("Incoming fragmented message but there is one already in progress")]
    FragmentedInProgress,

    #[error("Invalid continuation frame: no fragmented message to continue")]
    InvalidContinuationFrame,

    #[error("Invalid Opcode")]
    InvalidOpcode,

    // HTTP Errors
    #[error("{source}")]
    URLParseError {
        #[from]
        source: ParseError,
    },

    #[error("Invalid scheme in WebSocket URL")]
    InvalidSchemeURL,

    #[error("URL has no host")]
    URLNoHost,

    #[error("URL has no port")]
    URLNoPort,

    #[error("{source}")]
    HttpParseError {
        #[from]
        source: HttpParseError,
    },

    #[error("Incomplete HTTP request")]
    IncompleteHTTPRequest,

    // Domain addr parsing error
    #[error("{source}")]
    DomainError {
        #[from]
        source: InvalidDnsNameError,
    },

    #[error("use_tls = `{0}` argument does not match the passed URL scheme: `{1}`")]
    SchemeAgainstTlsConfig(bool, String),


    // Compression / Decompression Errors
    #[error("max_window_bits should be a value between 8 and 15")]
    InvalidMaxWindowBits
}
