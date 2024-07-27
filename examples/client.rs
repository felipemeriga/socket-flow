use tokio::net::TcpStream;
use tokio::select;
use simple_websocket::handshake::perform_client_handshake;
use tokio::time::{Duration, interval};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};


#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:9002").await.unwrap();

    let mut ws_connection = perform_client_handshake(stream).await.unwrap();

    let mut slow_ticker = interval(Duration::from_secs(3));

    loop {
        select! {
            Some(message) = ws_connection.read.recv() => {
                println!("Received message: {}", &String::from_utf8(message).unwrap())
            }
            _ = slow_ticker.tick() => {
                ws_connection.write.send(Vec::from(generate_random_string())).unwrap();
            }
        }
    }
}


fn generate_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}