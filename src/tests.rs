#[cfg(test)]
mod tests {
    use crate::frame::{Frame, OpCode};
    use crate::request::{construct_http_request, HttpRequest};

    use crate::extensions::{add_extension_headers, Extensions};
    use crate::handshake::{accept_async, accept_async_with_config, connect_async, connect_async_with_config, HTTP_ACCEPT_RESPONSE, SEC_WEBSOCKET_KEY};
    use crate::stream::SocketFlowStream;
    use crate::utils::generate_websocket_accept_value;
    use futures::StreamExt;
    use std::error::Error;
    use bytes::BytesMut;
    use rand::Rng;
    use tokio::io::{split, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{TcpListener, TcpStream};
    use serde::Serialize;
    use crate::config::{ClientConfig, WebSocketConfig};
    use crate::decoder::Decoder;
    use crate::encoder::Encoder;
    use serde_json::json;

    #[test]
    fn test_opcode() {
        let byte = 0x0;
        let res = OpCode::from(byte).unwrap();
        assert_eq!(res, OpCode::Continue);

        let opcode = OpCode::Text;
        let op_byte = opcode.as_u8();
        assert_eq!(op_byte, 0x1);

        assert_eq!(OpCode::Close.is_control(), true);
        assert_eq!(OpCode::Text.is_control(), false);
    }

    #[test]
    fn test_frame() {
        let final_fragment = false;
        let opcode = OpCode::Text;
        let payload: Vec<u8> = Vec::new();
        let frame = Frame::new(final_fragment, opcode.clone(), payload.clone(), false);

        assert_eq!(frame.final_fragment, final_fragment);
        assert_eq!(frame.opcode, opcode);
        assert_eq!(frame.payload, payload);
    }

    #[test]
    fn test_parse_to_http_request_valid() {
        let (request, host_with_port, host, use_tls) =
            construct_http_request("ws://localhost:8080", "dGhlIHNhbXBsZSBub25jZQ==", None).unwrap();
        assert_eq!(host_with_port, "localhost:8080");
        assert_eq!(host, "localhost");
        assert_eq!(use_tls, false);
        assert!(request.starts_with("GET / HTTP/1.1"));
        assert!(request.contains("Host: localhost"));
        assert!(request.contains("Upgrade: websocket"));
        assert!(request.contains("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ=="));
    }

    #[test]
    fn test_parse_to_http_request_invalid_scheme() {
        let result = construct_http_request("ftp://localhost:8080", "dGhlIHNhbXBsZSBub25jZQ==", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_to_http_request_no_host() {
        let result = construct_http_request("ws://:8080", "dGhlIHNhbXBsZSBub25jZQ==", None);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_accept_async() -> Result<(), Box<dyn Error>> {
        // Start a TCP listener (server) to accept a connection
        let listener = TcpListener::bind("127.0.0.1:0").await?; // bind to an available port
        let addr = listener.local_addr()?; // get the local address for the client to connect to

        // Simulate the client in a separate task
        let client = tokio::spawn(async move {
            // Client creates a connection to the server
            let mut stream = TcpStream::connect(addr).await.unwrap();

            // Send a mock WebSocket handshake request to the server
            let handshake_request = "GET / HTTP/1.1\r\n\
                                Host: 127.0.0.1\r\n\
                                Upgrade: websocket\r\n\
                                Connection: Upgrade\r\n\
                                Sec-WebSocket-Key: SGVsbG8sIHdvcmxkIQ==\r\n\
                                Sec-WebSocket-Version: 13\r\n\r\n";
            stream
                .write_all(handshake_request.as_bytes())
                .await
                .unwrap();

            // Read the response from the server (the server's handshake response)
            let mut buf = [0; 1024];
            let n = stream.read(&mut buf).await.unwrap();

            // Optional: Assert that the server sent a valid WebSocket handshake response
            let response = String::from_utf8_lossy(&buf[..n]);
            assert!(response.contains("HTTP/1.1 101 Switching Protocols"));
        });

        // Simulate the server accepting a connection
        let (socket, _) = listener.accept().await?;

        // Call your async WebSocket handshake function
        accept_async(SocketFlowStream::Plain(socket)).await?;

        // Ensure the client finishes
        client.await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_connect_async() -> Result<(), Box<dyn Error>> {
        // Start a TCP listener (server) to accept a connection
        let listener = TcpListener::bind("127.0.0.1:9005").await?; // bind to an available port

        // Simulate the server in a separate task
        let server = tokio::spawn(async move {
            // There is no need to split the stream, since we are doing everything inside this task
            let (stream, _) = listener.accept().await.unwrap();

            let (read, mut write) = split(stream);
            let mut buf_reader = BufReader::new(read);

            let mut req = HttpRequest::parse_http_request(&mut buf_reader)
                .await
                .unwrap();

            // Already testing get_header_value func, since it's easier than spliting the string to
            // find sec_websocket_key
            let sec_websocket_key = req.get_header_value(SEC_WEBSOCKET_KEY).unwrap();

            let accept_key = generate_websocket_accept_value(sec_websocket_key);

            let mut response = HTTP_ACCEPT_RESPONSE.replace("{}", &accept_key);
            add_extension_headers(&mut response, None);
            write.write_all(response.as_bytes()).await.unwrap();
            write.flush().await.unwrap();
        });

        // Call the connect_async function for connecting to the server
        connect_async("ws://127.0.0.1:9005").await?;

        server.await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_text_message() -> Result<(), Box<dyn Error>> {
        // Message to be sent by client
        const MESSAGE: &str = "TEST";

        // Start a TCP listener (server) to accept a connection
        let listener = TcpListener::bind("127.0.0.1:9006").await?; // bind to an available port

        tokio::spawn(async move {
            // Connect to the endpoint and send a simple text message
            let mut client_connection = connect_async("ws://127.0.0.1:9006").await.unwrap();
            client_connection
                .send(String::from(MESSAGE).into_bytes())
                .await
                .unwrap();
        });

        // Accept the connection and generate server connection
        let (stream, _) = listener.accept().await?;
        let mut server_connection = accept_async(SocketFlowStream::Plain(stream)).await?;

        // Wait to receive message from client
        while let Some(result) = server_connection.next().await {
            match result {
                Ok(message) => {
                    assert_eq!(
                        message.as_text()?,
                        String::from(MESSAGE),
                        "{}",
                        format!("Message receive from client should be: {}", MESSAGE)
                    );
                    break;
                }
                Err(err) => Err(err)?,
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_accept_and_connect() -> Result<(), Box<dyn Error>> {
        // Start a TCP listener (server) to accept a connection
        let listener = TcpListener::bind("127.0.0.1:9007").await?; // bind to an available port
        // payload to validate the message
        let payload = vec![1, 2, 3, 4];

        // Simulate the server in a separate task
        let payload_clone = payload.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();

            let mut server_connection = accept_async(SocketFlowStream::Plain(stream)).await.unwrap();
            if let Some(result) = server_connection.next().await {
                match result {
                    Ok(message) => assert_eq!(message.as_binary(), payload_clone),
                    Err(e) => panic!("Error occurred: {:?}", e),
                };
            }
        });

        // Call the connect_async function for connecting to the server
        let mut client_connection = connect_async("ws://127.0.0.1:9007").await?;
        // send the payload
        client_connection.send(payload).await.unwrap();
        client_connection.close_connection().await.unwrap();

        server.await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_compress_decompress_payload_reset_context() -> Result<(), Box<dyn Error>> {
        let payload = vec![1, 2, 3, 4, 5];

        let mut encoder = Encoder::new(true, Some(15));
        let mut decoder = Decoder::new(true, Some(15));

        let encoded_data = encoder.compress(&mut BytesMut::from(&payload[..]))?;
        let decoded_data = decoder.decompress(&mut BytesMut::from(&encoded_data[..]))?;

        assert_eq!(payload, decoded_data);
        Ok(())
    }

    #[tokio::test]
    async fn test_compress_decompress_payload_keep_context() -> Result<(), Box<dyn Error>> {
        let payload = vec![1, 2, 3, 4, 5];

        let mut encoder = Encoder::new(false, Some(15));
        let mut decoder = Decoder::new(false, Some(15));

        let encoded_data = encoder.compress(&mut BytesMut::from(&payload[..]))?;
        let _ = decoder.decompress(&mut BytesMut::from(&encoded_data[..]))?;

        let _ = encoder.compress(&mut BytesMut::from(&payload[..]))?;
        let second_decoded_data = decoder.decompress(&mut BytesMut::from(&encoded_data[..]))?;

        assert_eq!(payload, second_decoded_data);
        Ok(())
    }

    #[derive(Serialize)]
    struct User {
        id: i32,
        name: String,
    }
    fn generate_users() -> Vec<u8> {
        let mut users: Vec<User> = Vec::new();

        for id in 0..500 {
            let name: String = rand::rng()
                .sample_iter(&rand::distr::Alphanumeric)
                .take(30)
                .map(char::from)
                .collect();

            users.push(User { id, name });
        }

        let json = json!(&users);
        let serialized = serde_json::to_string(&json).unwrap();
        serialized.into_bytes()
    }

    #[tokio::test]
    async fn test_accept_and_connect_extension() -> Result<(), Box<dyn Error>> {
        // Start a TCP listener (server) to accept a connection
        let listener = TcpListener::bind("127.0.0.1:9008").await?; // bind to an available port
        let payload = generate_users();

        // Simulate the server in a separate task
        let payload_clone = payload.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut config = WebSocketConfig::default();
            config.extensions = Some(Extensions {
                permessage_deflate: true,
                client_no_context_takeover: Some(true),
                server_no_context_takeover: Some(true),
                client_max_window_bits: None,
                server_max_window_bits: None,
            });

            let mut server_connection = accept_async_with_config(SocketFlowStream::Plain(stream), Some(config)).await.unwrap();
            if let Some(result) = server_connection.next().await {
                match result {
                    Ok(message) => assert_eq!(message.as_binary(), payload_clone),
                    Err(e) => panic!("Error occurred: {:?}", e),
                };
            }
        });

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

        // Call the connect_async function for connecting to the server
        let mut client_connection = connect_async_with_config("ws://127.0.0.1:9008", Some(client_config)).await?;
        // send the payload
        client_connection.send(payload).await.unwrap();
        client_connection.close_connection().await.unwrap();

        server.await?;
        Ok(())
    }
}
