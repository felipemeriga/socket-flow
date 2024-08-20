use crate::connection::WSConnection;
use crate::error::Error;
use crate::frame::Frame;
use crate::read::ReadStream;
use crate::request::parse_to_http_request;
use crate::write::{Writer, WriterKind};
use base64::prelude::BASE64_STANDARD;
use base64::prelude::*;
use bytes::BytesMut;
use rand::random;
use sha1::{Digest, Sha1};
use std::sync::Arc;
use tokio::io::{split, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc::channel;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_stream::wrappers::ReceiverStream;

const SEC_WEBSOCKETS_KEY: &str = "Sec-WebSocket-Key:";
const UUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const SWITCHING_PROTOCOLS: &str = "101 Switching Protocols";

const HTTP_ACCEPT_RESPONSE: &str = "HTTP/1.1 101 Switching Protocols\r\n\
        Connection: Upgrade\r\n\
        Upgrade: websocket\r\n\
        Sec-WebSocket-Accept: {}\r\n\
        \r\n";

pub type Result = std::result::Result<WSConnection, Error>;

/// Used for accepting websocket connections as a server.
///
/// It basically does the first step of verifying the client key in the request
/// going to the second step, which is sending the accept response,
/// finally creating the connection, and returning a `WSConnection`
pub async fn accept_async(stream: TcpStream) -> Result {
    let (reader, mut write_half) = split(stream);
    let mut buf_reader = BufReader::new(reader);

    let sec_websockets_accept = header_read(&mut buf_reader).await;

    match sec_websockets_accept {
        Some(accept_value) => {
            let response = HTTP_ACCEPT_RESPONSE.replace("{}", &accept_value);
            write_half
                .write_all(response.as_bytes())
                .await
                .map_err(|source| Error::IOError { source })?;
            write_half.flush().await?;
        }
        None => Err(Error::NoSecWebsocketKey)?,
    }

    second_stage_handshake(buf_reader, write_half, WriterKind::Server).await
}

async fn second_stage_handshake(
    buf_reader: BufReader<ReadHalf<TcpStream>>,
    write_half: WriteHalf<TcpStream>,
    kind: WriterKind,
) -> Result {
    // This writer instance would be used for writing frames into the socket.
    // Since it's going to be used by two different instances, we need to wrap it through an Arc
    let writer = Arc::new(Mutex::new(Writer::new(write_half, kind)));

    let stream_writer = writer.clone();

    // ReadStream will be running on a separate task, capturing all the incoming frames from the connection, and broadcasting them through this
    // tokio mpsc channel. Therefore, it can be consumed by the end-user of this library
    let (read_tx, read_rx) = channel::<std::result::Result<Frame, Error>>(20);
    let mut read_stream = ReadStream::new(buf_reader, read_tx, stream_writer);

    let connection_writer = writer.clone();
    // Transforming the receiver of the channel into a Stream, so we could leverage using
    // next() method, for processing the values from this channel
    let receiver_stream = ReceiverStream::new(read_rx);

    // The WSConnection is the structure that will be delivered to the end-user, which contains
    // a stream of frames, for consuming the incoming frames, and methods for writing frames into
    // the socket
    let ws_connection = WSConnection::new(connection_writer, receiver_stream);

    // Spawning poll_messages which is the method for reading the frames from the socket concurrently,
    // because we need this method running, while the end-user can have
    // a connection returned, for receiving and sending messages.
    // Since this is the only task that holds the ownership of BufReader, if some IO error happens,
    // poll_messages will return.
    // BufReader will be dropped, hence, the writeHalf and TCP connection
    tokio::spawn(async move {
        if let Err(err) = read_stream.poll_messages().await {
            let _ = read_stream.read_tx.send(Err(err)).await;
        }
    });

    Ok(ws_connection)
}

/// Used for connecting as a client to a websocket endpoint.
///
/// It basically does the first step of genereating the client key
/// going to the second step, which is parsing the server reponse,
/// finally creating the connection, and returning a `WSConnection`
pub async fn connect_async(addr: &str) -> Result {
    let client_websocket_key = generate_websocket_key();
    let (request, hostname) = parse_to_http_request(addr, &client_websocket_key)?;

    let stream = TcpStream::connect(hostname).await?;

    let (reader, mut write_half) = split(stream);
    let mut buf_reader = BufReader::new(reader);

    write_half.write_all(request.as_bytes()).await?;

    // Create a buffer for the server's response, since most of the Websocket won't send a big payload
    // for the handshake response, defining this size of Vector would be enough, and also will put a limit
    // to bigger payloads
    let mut buffer: Vec<u8> = vec![0; 206];

    // Read the server's response
    let number_read = buf_reader.read(&mut buffer).await?;

    // Keep only the section of the buffer that was filled.
    buffer.truncate(number_read);

    // Convert the server's response to a string
    let response = String::from_utf8(buffer)?;

    // Verify that the server agreed to upgrade the connection
    if !response.contains(SWITCHING_PROTOCOLS) {
        return Err(Error::NoUpgrade);
    }

    // Generate the server expected accept key using UUID, and checking if it's present in the response
    let expected_accept_value = generate_websocket_accept_value(client_websocket_key);
    if !response.contains(&expected_accept_value) {
        return Err(Error::InvalidAcceptKey);
    }

    second_stage_handshake(buf_reader, write_half, WriterKind::Client).await
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
