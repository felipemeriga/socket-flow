use futures::StreamExt;
use log::*;
use rand::distr::Alphanumeric;
use rand::{thread_rng, Rng};
use socket_flow::config::ClientConfig;
use socket_flow::handshake::connect_async_with_config;
use tokio::select;
use tokio::time::{interval, Duration};

async fn handle_connection(addr: &str) {
    let mut client_config = ClientConfig::default();
    client_config.ca_file = Some(String::from("ca.crt"));

    match connect_async_with_config(addr, Some(client_config)).await {
        Ok(mut ws_connection) => {
            let mut ticker = interval(Duration::from_secs(5));
            // it will be used for closing the connection
            let mut counter = 0;

            loop {
                select! {
                    Some(result) = ws_connection.next() => {
                        match result {
                            Ok(message) => {
                                 info!("Received message: {}", message.as_text().unwrap());
                                counter = counter + 1;
                                // close the connection if 3 messages have already been sent and received
                                if counter >= 3 {
                                    if ws_connection.close_connection().await.is_err() {
                                         error!("Error occurred when closing connection");
                                    }
                                    break;
                                }
                            }
                            Err(err) => {
                                error!("Received error from the stream: {}", err);

                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        let random_string = generate_random_string();
                        let binary_data = Vec::from(random_string);

                        if ws_connection.send(binary_data).await.is_err() {
                            eprintln!("Failed to send message");
                            break;
                        }
                    }
                }
            }
        }
        Err(err) => error!("Error when performing handshake: {}", err),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    handle_connection("wss://localhost:9002").await;
}

fn generate_random_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}
