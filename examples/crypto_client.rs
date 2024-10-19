use futures::StreamExt;
use log::*;
use socket_flow::handshake::connect_async;

async fn handle_connection(addr: &str) {
    match connect_async(addr).await {
        Ok(mut ws_connection) => {
            while let Some(result) = ws_connection.next().await {
                match result {
                    Ok(message) => {
                        info!("Received message: {}", message.as_text().unwrap());
                    }
                    Err(e) => {
                        error!("Received error from the stream: {}", e);
                        break;
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
    handle_connection("wss://api.gemini.com/v1/marketdata/BTCUSD").await;
}
