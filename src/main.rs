use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, split};
use sha1::{Digest, Sha1};
use std::{io, str};
use bytes::BytesMut;
use tokio::time::{timeout, Duration};

#[tokio::main]
pub async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let (mut reader, mut writer) = split(socket);
            let mut buf_reader = BufReader::new(reader);

            let mut websocket_key = None;
            let mut buf = BytesMut::with_capacity(1024 * 16); // 16 kilobytes

            // Limit the maximum amount of data read to prevent a denial of service attack.
            while buf.len() <= 1024 * 16 {
                let mut tmp_buf = vec![0; 1024];
                match timeout(Duration::from_secs(5), buf_reader.read(&mut tmp_buf)).await {
                    Ok(Ok(0)) | Err(_) => break, // EOF reached or Timeout, we stop.
                    Ok(Ok(n)) => {
                        buf.extend_from_slice(&tmp_buf[..n]);
                        let s = String::from_utf8_lossy(&buf);
                        if let Some(start) = s.find("Sec-WebSocket-Key:") {
                            websocket_key = Some(s[start..].lines().next().unwrap().to_string());
                            break;
                        }
                    },
                    _ => {},
                }
            }

            if let Some(key) = websocket_key {
                // Process the key line to extract the actual key value
                let key_value = parse_websocket_key(key);
                let accept_value = generate_websocket_accept_value(key_value.unwrap());

                let response = format!(
                    "HTTP/1.1 101 Switching Protocols\r\n\
        Connection: Upgrade\r\n\
        Upgrade: websocket\r\n\
        Sec-WebSocket-Accept: {}\r\n\
        \r\n", accept_value);
                writer.write_all(response.as_bytes()).await.unwrap();
            }
        });
    }
}

fn parse_websocket_key(first_request: String) -> Option<String> {
    for line in first_request.lines() {
        if line.starts_with("Sec-WebSocket-Key: ") {
            return line[18..].split_whitespace().next().map(ToOwned::to_owned);
        }
    }
    None
}

fn generate_websocket_accept_value(key: String) -> String {
    let mut sha1 = Sha1::new();
    sha1.update(key.as_bytes());
    sha1.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());
    base64::encode(sha1.finalize())
}