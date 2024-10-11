use futures::StreamExt;
use log::*;
use socket_flow::error::Error;
use socket_flow::handshake::connect_async;

const AGENT: &str = "socket-flow";

async fn run_test(case: u32) -> Result<(), Error> {
    info!("Running test case {}", case);
    let case_url = &format!("ws://127.0.0.1:9001/runCase?case={}&agent={}", case, AGENT);
    let mut connection = connect_async(case_url, None).await?;
    while let Some(msg) = connection.next().await {
        let msg = msg?;
        connection.send_message(msg).await?;
    }

    Ok(())
}

async fn update_reports() -> Result<(), Error> {
    info!("updating reports");
    let mut connection = connect_async(
        &format!("ws://127.0.0.1:9001/updateReports?agent={}", AGENT),
        None,
    )
    .await?;
    info!("closing connection");
    connection.close_connection().await?;
    Ok(())
}

async fn get_case_count() -> Result<u32, Error> {
    let mut connection = connect_async("ws://localhost:9001/getCaseCount", None).await?;

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
