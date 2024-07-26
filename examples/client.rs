use tokio::net::TcpStream;
use simple_websocket::handshake::perform_client_handshake;

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:9002").await.unwrap();

    perform_client_handshake(stream).await;
}