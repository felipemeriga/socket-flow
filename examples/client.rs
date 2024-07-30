use tokio::net::TcpStream;
use tokio::select;
use simple_websocket::handshake::perform_client_handshake;
use tokio::time::{Duration, Instant, interval, interval_at};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use simple_websocket::frame::{Frame, OpCode};

async fn handle_connection(stream: TcpStream) {
    match perform_client_handshake(stream).await {
        Ok(mut ws_connection) => {
            let mut ticker = interval(Duration::from_secs(10));

            let start = Instant::now() + Duration::from_secs(11);
            let mut close_ticker = interval_at(start, Duration::from_secs(30));

            loop {
                // SOLUTION - The racing condition was being caused due to the use of unbounded channels
                // which don't implement a future, which is a sync operation, that is why the stream was breaking
                // even before sending the close message, change all channels to normal bounded tokio channels
                // Additionally, based on Websockets RFC 6455, when a client sends a close opcode, the server needs to
                // answer with another close, and the client can close the connection once it receives the close.
                select! {
                    Some(result) = ws_connection.read.recv() => {
                        match result {
                            Ok(message) => {
                                 println!("Received message: {}", &String::from_utf8(message).unwrap())
                            }
                            Err(err) => {
                                eprintln!("Received error from the stream: {}", err);
                                break;
                            }
                        }
                    }
                    // _ = ticker.tick() => {
                    //     let random_string = generate_random_string();
                    //     let binary_data = Vec::from(random_string);
                    //     if ws_connection.write.send(Frame::new(true, OpCode::Text, binary_data)).await.is_err() {
                    //         eprintln!("Failed to send message");
                    //         break;
                    //     }
                    // }
                    _ = close_ticker.tick() => {
                        ws_connection.close_connection().await;
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