use tokio::net::TcpStream;
use tokio::select;
use simple_websocket::handshake::perform_client_handshake;
use tokio::time::{Duration, interval};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};


async fn handle_connection(stream: TcpStream) {
    match perform_client_handshake(stream).await {
        Ok(mut ws_connection) => {
            let mut ticker = interval(Duration::from_secs(3));

            loop {
                select! {
                    Some(message) = ws_connection.read.recv() => {
                        println!("Received message: {}", &String::from_utf8(message).unwrap())
                    }
                    _ = ticker.tick() => {
                        let random_string = generate_random_string();
                        let binary_data = Vec::from(random_string);
                        if ws_connection.write.send(binary_data).is_err() {
                            eprintln!("Failed to send message");
                            break;
                        }
                    }
                }
            }
        }
        Err(err) => eprintln!("Error when performing handshake: {}", err)
    }
}

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:9002").await.expect("Couldn't connect to the server");

    handle_connection(stream).await;
}


fn generate_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}