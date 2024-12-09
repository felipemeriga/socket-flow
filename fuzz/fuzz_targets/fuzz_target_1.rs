#![no_main]

use libfuzzer_sys::fuzz_target;
use socket_flow::handshake::accept_async;
use socket_flow::stream::SocketFlowStream;
use tokio::runtime::Runtime;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncWriteExt;
use rand::Rng; // For generating random data
use std::str;

fuzz_target!(|data: &[u8]| {
    let runtime = Runtime::new().unwrap();

    let data_vec = Vec::from(data);
    runtime.block_on(async move {
        // Create a local TCP listener.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn a task to accept the incoming connection and handle fuzz data.
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // Construct a WebSocket handshake with fuzzed data.
                let mut handshake = build_fuzzed_handshake(&data_vec);

                // Write fuzzed handshake data into the socket for the server to read.
                let _ = socket.write_all(&handshake).await;
            }
        });

        // Connect to the listener using TcpStream.
        if let Ok(client_stream) = TcpStream::connect(addr).await {
            // Wrap the client stream in a SocketFlowStream.
            let stream = SocketFlowStream::Plain(client_stream);

            // Test the handshake function with the fuzzed input.
            let result = accept_async(stream).await;

            match result {
                Err(err) => println!("{:?}", err),
            _ => {}}
        }
    });
});

// Helper function to build a fuzzed WebSocket handshake request
fn build_fuzzed_handshake(data: &[u8]) -> Vec<u8> {
    // Start with a basic WebSocket handshake template
    let mut handshake = b"GET / HTTP/1.1\r\n\
                          Host: 127.0.0.1\r\n\
                          Upgrade: websocket\r\n\
                          Connection: Upgrade\r\n".to_vec();

    // Append a fuzzed Sec-WebSocket-Key
    let key = generate_fuzzed_key(data);
    handshake.push_str(&format!("Sec-WebSocket-Key: {}\r\n", key));

    // Append a fixed Sec-WebSocket-Version for now (this can be fuzzed as well)
    handshake.push_str("Sec-WebSocket-Version: 13\r\n");

    // Optionally, fuzz headers like `Connection` or `Host`
    if data.len() % 2 == 0 {
        handshake.push_str("Connection: Fuzzed-Value\r\n");
    }

    // End the headers with the necessary blank line
    handshake.push_str("\r\n");

    handshake
}

// Helper function to generate a random Sec-WebSocket-Key from fuzz data
fn generate_fuzzed_key(data: &[u8]) -> String {
    // Here you can use the fuzz data to generate a random base64 string
    let random_key = base64::encode(data);
    random_key
}
