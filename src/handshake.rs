use crate::connection::WSConnection;
use crate::read::{ReadStream};
use base64::prelude::BASE64_STANDARD;
use base64::prelude::*;
use bytes::BytesMut;
use sha1::{Digest, Sha1};
use thiserror::Error;
use tokio::io;
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::{timeout, Duration};
use crate::frame::Frame;
use crate::write::WriteStream;

const SEC_WEBSOCKETS_KEY: &str = "Sec-WebSocket-Key:";
const UUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

const HTTP_ACCEPT_RESPONSE: &str = "HTTP/1.1 101 Switching Protocols\r\n\
        Connection: Upgrade\r\n\
        Upgrade: websocket\r\n\
        Sec-WebSocket-Accept: {}\r\n\
        \r\n";

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
pub type Result = std::result::Result<WSConnection, HandshakeError>;

// Using Send trait because we are going to run the process to read frames from the socket concurrently
// TCPStream from tokio implements Send
// Using static, because tokio::spawn returns a JoinHandle, because the spawned task could outilive the
// lifetime of the function call to tokio::spawn.
pub async fn perform_handshake<T: AsyncRead + AsyncWrite + Send + 'static>(stream: T) -> Result {
    let (reader, mut writer) = split(stream);
    let mut buf_reader = BufReader::new(reader);

    let sec_websockets_accept = header_read(&mut buf_reader).await;

    match sec_websockets_accept {
        Some(accept_value) => {
            let response = HTTP_ACCEPT_RESPONSE.replace("{}", &accept_value);
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(|source| HandshakeError::IOError { source })?
        }
        None => Err(HandshakeError::NoSecWebsocketKey)?,
    }

    // We are using unbounded async channels to communicate the frames received from the client
    // and another channel to send messages from server to client
    let (write_tx, write_rx) = unbounded_channel();
    let (read_tx, read_rx) = unbounded_channel();

    let (internal_tx, internal_rx) = unbounded_channel::<Frame>();

    let mut read_stream = ReadStream::new(buf_reader, read_tx, internal_tx);
    let mut write_stream = WriteStream::new(writer, write_rx, internal_rx);

    let ws_connection = WSConnection::new(read_rx, write_tx);

    // We are spawning poll_messages which is the method for reading the frames from the socket
    // we need to do it concurrently, because we need this method running, while the end-user can have
    // a channel returned, for receiving and sending messages
    // Since ReadHalf and WriteHalf implements Send and Sync, it's ok to send them over spawn
    // Additionally, since our BufReader doesn't change, we only call read methods from it, there is no
    // need to wrap it in an Arc<Mutex>, also because poll_messages read frames sequentially.
    tokio::spawn(async move {
        match read_stream.poll_messages().await {
            Err(err) => {
                eprintln!("Received error from stream: {}", err)
            }
            _ => {}
        }
    });

    tokio::spawn(async move {
        write_stream.run().await;
    });

    Ok(ws_connection)
}

// Here we are using the generic T, and expressing its two tokio traits, to avoiding adding the
// entire type of the argument in the function signature (BufReader<ReadHalf<TcpStream>>)
// The Unpin trait in Rust is used when the exact location of an object in memory needs to remain
// constant after being pinned. In simple terms, it means that the object doesn't move around in memory
// Here, we need to use Unpin, because the timeout function puts the passed Future into a Pin<Box<dyn Future>>
async fn header_read<T: AsyncReadExt + Unpin>(buf_reader: &mut T) -> Option<String> {
    let mut websocket_header: Option<String> = None;
    let mut websocket_accept: Option<String> = None;
    let mut header_buf = BytesMut::with_capacity(1024 * 16); // 16 kilobytes

    // Limit the maximum amount of data read to prevent a denial of service attack.
    while header_buf.len() <= 1024 * 16 {
        let mut tmp_buf = vec![0; 1024];
        match timeout(Duration::from_secs(10), buf_reader.read(&mut tmp_buf)).await {
            Ok(Ok(0)) | Err(_) => break, // EOF reached or Timeout, we stop. In the case of EOF
            // there is no need to log or return EOF or timeout errors
            Ok(Ok(n)) => {
                header_buf.extend_from_slice(&tmp_buf[..n]);
                let s = String::from_utf8_lossy(&header_buf);
                if let Some(start) = s.find(SEC_WEBSOCKETS_KEY) {
                    websocket_header = Some(s[start..].lines().next().unwrap().to_string());
                    break;
                }
            }
            _ => {}
        }
    }

    match websocket_header {
        Some(header) => {
            let key_value = parse_websocket_key(header);
            match key_value {
                Some(key) => {
                    websocket_accept = Some(generate_websocket_accept_value(key));
                }
                _ => {}
            }
        }
        _ => {}
    }

    websocket_accept
}

fn parse_websocket_key(header: String) -> Option<String> {
    for line in header.lines() {
        if line.starts_with(SEC_WEBSOCKETS_KEY) {
            return line[18..].split_whitespace().next().map(ToOwned::to_owned);
        }
    }
    None
}

fn generate_websocket_accept_value(key: String) -> String {
    let mut sha1 = Sha1::new();
    sha1.update(key.as_bytes());
    sha1.update(UUID.as_bytes());
    BASE64_STANDARD.encode(sha1.finalize())
}
