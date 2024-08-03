use rand::distr::Alphanumeric;
use rand::{Rng, thread_rng};
use tokio::net::TcpStream;
use simple_websocket::handshake::perform_client_handshake;

async fn handle_connection(stream: TcpStream) {
    match perform_client_handshake(stream).await {
        Ok(mut ws_connection) => {
            let my_random_string = generate_random_string();
            println!("Sending random string: {}", my_random_string);
            if ws_connection.send_large_data_fragmented(Vec::from(my_random_string)).await.is_err() {
                eprintln!("Error occurred when sending data in chunks");
            }

            ws_connection.close_connection().await.unwrap();
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