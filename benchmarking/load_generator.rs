use tokio::sync::mpsc;
use std::time::{Duration, Instant};
use tokio_stream::StreamExt;
use socket_flow::handshake::connect_async;

#[tokio::main]
async fn main() {
    let url = "ws://127.0.0.1:9002";
    let connection_count = 100; // Number of WebSocket clients
    let message_count = 1000;  // Messages per client
    let message_size = 16384;   // Size of each message in bytes

    let (tx, mut rx) = mpsc::unbounded_channel();

    for _ in 0..connection_count {
        let tx = tx.clone();
        tokio::spawn(async move {
            let ws_connection = connect_async(url).await.unwrap();
            let (mut read, mut write) = ws_connection.split();

            let payload = vec![b'a'; message_size];
            let start = Instant::now();

            for _ in 0..message_count {
                write.send(payload.clone()).await.unwrap();
                let _ = read.next().await.unwrap();
            }

            let duration = start.elapsed();
            tx.send(duration).unwrap();
        });
    }

    drop(tx); // Close the channel

    let mut total_duration = Duration::new(0, 0);
    while let Some(duration) = rx.recv().await {
        total_duration += duration;
    }

    let avg_latency = total_duration / (connection_count as u32 * message_count as u32);
    println!("Average Latency: {:?}", avg_latency);
}
