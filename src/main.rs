use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use sha1::{Digest, Sha1};
use std::{io, str};


#[tokio::main]
pub async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9000").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            // WebSocket handshake
            const RESPONSE: &'static str = "\
            HTTP/1.1 101 Switching Protocols\r\n\
            Upgrade: websocket\r\n\
            Connection: Upgrade\r\n\
            Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\
            \r\n";

            let mut buf = vec![0; 1024];
            socket.read(&mut buf).await.unwrap();
            let sec_websocket_key = parse_websocket_key(&buf);
            let accept_value = generate_websocket_accept_value(&sec_websocket_key.unwrap());
            let response = format!(
                "HTTP/1.1 101 Switching Protocols\r\n\
            Connection: Upgrade\r\n\
            Upgrade: websocket\r\n\
            Sec-WebSocket-Accept: {}\r\n\
            \r\n", accept_value);
            socket.write_all(response.as_bytes()).await.unwrap();


            socket.read(&mut buf).await.unwrap();
            // if request.contains("Upgrade: websocket") {
            //     socket.write_all(RESPONSE.as_bytes()).await.unwrap();
            // }
        });
    }
}

fn parse_websocket_key(request: &[u8]) -> Option<String> {
    let request = str::from_utf8(request).ok()?;
    for line in request.lines() {
        if line.starts_with("Sec-WebSocket-Key: ") {
            return line[18..].split_whitespace().next().map(ToOwned::to_owned);
        }
    }
    None
}

fn generate_websocket_accept_value(key: &str) -> String {
    let mut sha1 = Sha1::new();
    sha1.update(key.as_bytes());
    sha1.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());
    base64::encode(sha1.finalize())
}