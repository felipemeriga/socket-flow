use futures::StreamExt;
use log::*;
use socket_flow::config::{ClientConfig, WebSocketConfig};
use socket_flow::error::Error;
use socket_flow::extensions::Extensions;
use socket_flow::handshake::{connect_async_with_config};

const AGENT: &str = "socket-flow";

fn get_config() -> ClientConfig {
    let mut websocket_config = WebSocketConfig::default();
    websocket_config.extensions = Some(Extensions {
        permessage_deflate: true,
        client_no_context_takeover: Some(true),
        server_no_context_takeover: Some(true),
        client_max_window_bits: None,
        server_max_window_bits: None,
    });
    let mut client_config = ClientConfig::default();
    client_config.web_socket_config = websocket_config;
    client_config
}

async fn run_test(case: u32) -> Result<(), Error> {
    let config = get_config();

    info!("Running test case {}", case);
    let case_url = &format!("ws://127.0.0.1:9001/runCase?case={}&agent={}", case, AGENT);
    let mut connection = connect_async_with_config(case_url, Some(config)).await?;
    while let Some(msg) = connection.next().await {
        let msg = msg?;
        connection.send_message(msg).await?;
    }

    Ok(())
}

async fn update_reports() -> Result<(), Error> {
    let config = get_config();

    info!("updating reports");
    let mut connection = connect_async_with_config(&format!(
        "ws://127.0.0.1:9001/updateReports?agent={}",
        AGENT
    ), Some(config))
    .await?;
    info!("closing connection");
    connection.close_connection().await?;
    Ok(())
}

async fn get_case_count() -> Result<u32, Error> {
    let config = get_config();

    let mut connection = connect_async_with_config("ws://localhost:9001/getCaseCount", Some(config)).await?;

    // Receive a single message
    let msg = connection.next().await.unwrap()?;
    connection.close_connection().await?;

    let text_message = msg.as_text()?;
    Ok(text_message
        .parse::<u32>()
        .expect("couldn't convert test case to number"))
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let total = get_case_count().await.expect("Error getting case count");

    for case in 1..=total {
        if let Err(e) = run_test(case).await {
            error!("Testcase {} failed: {}", case, e)
        }
    }

    update_reports().await.expect("Error updating reports");
}
