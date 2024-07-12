use base64::prelude::BASE64_STANDARD;
use thiserror::Error;
use bytes::BytesMut;
use base64::prelude::*;
use sha1::{Digest, Sha1};
use tokio::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter, ReadHalf, split};
use tokio::time::{timeout, Duration};
use crate::stream::WebsocketsStream;


const SEC_WEBSOCKETS_KEY: &str = "Sec-WebSocket-Key:";
const UUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

const HTTP_ACCEPT_RESPONSE: &str = "HTTP/1.1 101 Switching Protocols\r\n\
        Connection: Upgrade\r\n\
        Upgrade: websocket\r\n\
        Sec-WebSocket-Accept: {}\r\n\
        \r\n";
#[derive(Error, Debug)]
pub enum HandshakeError {
    #[error("No Sec-WebSocket-Key header present in the request")]
    NoSecWebsocketKey,

    #[error("Error when writing to socket: {source}")]
    WriteError {
        #[from]
        source: io::Error,
    },
}
pub type Result = std::result::Result<(), HandshakeError>;


pub async fn perform_handshake<T: AsyncRead + AsyncWrite>(stream: T) -> Result {
    let (reader, mut writer) = split(stream);
    let mut buf_reader = BufReader::new(reader);
    let mut buf_writer = BufWriter::new(writer);

    let sec_websockets_accept = header_read(&mut buf_reader).await;

    match sec_websockets_accept {
        Some(accept_value) => {
            let response = HTTP_ACCEPT_RESPONSE.replace("{}", &accept_value);
            buf_writer.write_all(response.as_bytes()).await.map_err(|source| HandshakeError::WriteError { source })?
        }
        None => Err(HandshakeError::NoSecWebsocketKey)?
    }

    let websockets_stream = WebsocketsStream {
        read: buf_reader,
        write: buf_writer,
    };

    Ok(())
}

// Here we are using the generic T, and expressing its two tokio traits, to avoiding adding the
// entire type of the argument in the function signature (BufReader<ReadHalf<TcpStream>>)
async fn header_read<T: AsyncReadExt + Unpin>(buf_reader: &mut T) -> Option<String> {
    let mut websocket_header: Option<String> = None;
    let mut websocket_accept: Option<String> = None;
    let mut header_buf = BytesMut::with_capacity(1024 * 16); // 16 kilobytes

    // Limit the maximum amount of data read to prevent a denial of service attack.
    while header_buf.len() <= 1024 * 16 {
        let mut tmp_buf = vec![0; 1024];
        match timeout(Duration::from_secs(10), buf_reader.read(&mut tmp_buf)).await {
            Ok(Ok(0)) | Err(_) => { break } // EOF reached or Timeout, we stop.
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