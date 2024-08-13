use rand::distr::Alphanumeric;
use rand::{thread_rng, Rng};
use socket_flow::handshake::perform_client_handshake;
use tokio::net::TcpStream;
use tokio::select;
use tokio::time::{interval, Duration};

async fn handle_connection(stream: TcpStream) {
    match perform_client_handshake(stream).await {
        Ok(mut ws_connection) => {
            let mut ticker = interval(Duration::from_secs(5));
            // it will be used for closing the connection
            let mut counter = 0;

            loop {
                select! {
                    Some(result) = ws_connection.read.recv() => {
                        match result {
                            Ok(frame) => {
                                 println!("Received message: {}", &String::from_utf8(frame.payload).unwrap());
                                counter = counter + 1;
                                // close the connection if 3 messages have already been sent and received
                                if counter >= 3 {
                                    if ws_connection.close_connection().await.is_err() {
                                         eprintln!("Error occurred when closing connection");
                                    }
                                    break;
                                }
                            }
                            Err(err) => {
                                eprintln!("Received error from the stream: {}", err);
                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        let random_string = generate_random_string();
                        let binary_data = Vec::from(random_string);

                        if ws_connection.send_data(binary_data).await.is_err() {
                            eprintln!("Failed to send message");
                            break;
                        }
                    }
                }
            }
        }
        Err(err) => eprintln!("Error when performing handshake: {}", err),
    }
}

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:9002")
        .await
        .expect("Couldn't connect to the server");

    handle_connection(stream).await;
}

fn generate_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}
