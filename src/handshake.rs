use crate::connection::WSConnection;
use crate::error::{HandshakeError, StreamError};
use crate::frame::Frame;
use crate::read::{ReadStream, StreamKind};
use crate::write::WriteStream;
use base64::prelude::BASE64_STANDARD;
use base64::prelude::*;
use bytes::BytesMut;
use rand::random;
use sha1::{Digest, Sha1};
use std::sync::Arc;
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::channel;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

const SEC_WEBSOCKETS_KEY: &str = "Sec-WebSocket-Key:";
const UUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const SWITCHING_PROTOCOLS: &str = "101 Switching Protocols";

const HTTP_ACCEPT_RESPONSE: &str = "HTTP/1.1 101 Switching Protocols\r\n\
        Connection: Upgrade\r\n\
        Upgrade: websocket\r\n\
        Sec-WebSocket-Accept: {}\r\n\
        \r\n";

const HTTP_HANDSHAKE_REQUEST: &str = "GET / HTTP/1.1\r\n\
        Host: {host}\r\n\
        Connection: Upgrade\r\n\
        Upgrade: websocket\r\n\
        Sec-WebSocket-Key: {key}\r\n\
        Sec-WebSocket-Version: 13\r\n\
        Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits\r\n\
        \r\n";
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

    second_stage_handshake(StreamKind::Server, buf_reader, writer).await
}

async fn second_stage_handshake<
    R: AsyncReadExt + Send + Unpin + 'static,
    W: AsyncWriteExt + Send + Unpin + 'static,
>(
    kind: StreamKind,
    buf_reader: R,
    writer: W,
) -> Result {
    // We are using tokio async channels to communicate the frames received from the client
    // and another channel to send messages from server to client
    // TODO - Check if 20 is a good number for Buffer size, remembering that channel is async, so if it's full
    // all the callers that are trying to add new data, will be blocked until we have free space (off course, using await in the method)
    let (write_tx, write_rx) = channel::<Frame>(20);

    let (read_tx, read_rx) = channel::<std::result::Result<Vec<u8>, StreamError>>(20);
    let read_tx = Arc::new(Mutex::new(read_tx));

    // These internal channels are used to communicate between write and read stream
    let (internal_tx, internal_rx) = channel::<Frame>(20);
    let (close_tx, close_rx) = channel::<bool>(1);

    let read_tx_stream = read_tx.clone();
    // We are separating the stream in read and write, because handling them in the same struct, would need us to
    // wrap some references with Arc<mutex>, and for the sake of a clean syntax, we selected to split it
    let mut read_stream = ReadStream::new(kind, buf_reader, read_tx_stream, internal_tx, close_tx);
    let mut write_stream = WriteStream::new(writer, write_rx, internal_rx);

    let ws_connection = WSConnection::new(read_rx, write_tx, close_rx);

    let read_tx_r = read_tx.clone();
    // We are spawning poll_messages which is the method for reading the frames from the socket
    // we need to do it concurrently, because we need this method running, while the end-user can have
    // a channel returned, for receiving and sending messages
    // Since ReadHalf and WriteHalf implements Send and Sync, it's ok to send them over spawn
    // Additionally, since our BufReader doesn't change, we only call read methods from it, there is no
    // need to wrap it in an Arc<Mutex>, also because poll_messages read frames sequentially.
    // Also, since this is the only task that holds the ownership of BufReader, if some IO error happens,
    // poll_messages will return, and since BufReader is only inside the scope of the function, it will be dropped
    // dropping the WriteHalf, hence, the TCP connection

    tokio::spawn(async move {
        if let Err(err) = read_stream.poll_messages().await {
            read_tx_r.lock().await.send(Err(err)).await.unwrap();
        }
    });

    let read_tx_w = read_tx.clone();
    tokio::spawn(async move {
        if let Err(err) = write_stream.run().await {
            read_tx_w.lock().await.send(Err(err)).await.unwrap()
        }
        drop(read_tx_w);
    });

    Ok(ws_connection)
}

pub async fn perform_client_handshake(stream: TcpStream) -> Result {
    let client_websocket_key = generate_websocket_key();
    let request = HTTP_HANDSHAKE_REQUEST
        .replace("{key}", &client_websocket_key)
        .replace("{host}", &stream.local_addr().unwrap().to_string());

    let (reader, mut writer) = split(stream);
    let mut buf_reader = BufReader::new(reader);

    writer.write_all(request.as_bytes()).await?;

    // Create a buffer for the server's response, since most of the Websocket won't send a big payload
    // for the handshake response, defining this size of Vector would be enough, and also will put a limit
    // to bigger payloads
    let mut buffer: Vec<u8> = vec![0; 1024];

    // Read the server's response
    let number_read = buf_reader.read(&mut buffer).await?;

    // Keep only the section of the buffer that was filled.
    buffer.truncate(number_read);

    // Convert the server's response to a string
    let response = String::from_utf8(buffer)?;

    // Verify that the server agreed to upgrade the connection
    if !response.contains(SWITCHING_PROTOCOLS) {
        return Err(HandshakeError::NoUpgrade);
    }

    // Generate the server expected accept key using UUID, and checking if it's present in the response
    let expected_accept_value = generate_websocket_accept_value(client_websocket_key);
    if !response.contains(&expected_accept_value) {
        return Err(HandshakeError::InvalidAcceptKey);
    }

    second_stage_handshake(StreamKind::Client, buf_reader, writer).await
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

    if let Some(header) = websocket_header {
        if let Some(key) = parse_websocket_key(header) {
            websocket_accept = Some(generate_websocket_accept_value(key));
        }
    }

    websocket_accept
}

fn parse_websocket_key(header: String) -> Option<String> {
    for line in header.lines() {
        if line.starts_with(SEC_WEBSOCKETS_KEY) {
            if let Some(stripped) = line.strip_prefix(SEC_WEBSOCKETS_KEY) {
                return stripped.split_whitespace().next().map(ToOwned::to_owned);
            }
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

fn generate_websocket_key() -> String {
    let random_bytes: [u8; 16] = random();
    BASE64_STANDARD.encode(random_bytes)
}
