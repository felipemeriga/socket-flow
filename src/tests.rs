#[cfg(test)]
mod tests {
    use crate::frame::{Frame, OpCode};
    use crate::request::{parse_to_http_request, RequestExt};

    use crate::handshake::{
        accept_async, connect_async, generate_websocket_accept_value, HTTP_ACCEPT_RESPONSE,
        SEC_WEBSOCKET_KEY,
    };
    use crate::stream::SocketFlowStream;
    use futures::StreamExt;
    use httparse::{Request, EMPTY_HEADER};
    use std::error::Error;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

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
        let frame = Frame::new(final_fragment, opcode.clone(), payload.clone());

        assert_eq!(frame.final_fragment, final_fragment);
        assert_eq!(frame.opcode, opcode);
        assert_eq!(frame.payload, payload);
    }

    #[test]
    fn test_parse_to_http_request_valid() {
        let (request, host_with_port, host, use_tls) =
            parse_to_http_request("ws://localhost:8080", "dGhlIHNhbXBsZSBub25jZQ==").unwrap();
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
        let result = parse_to_http_request("ftp://localhost:8080", "dGhlIHNhbXBsZSBub25jZQ==");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_to_http_request_no_host() {
        let result = parse_to_http_request("ws://:8080", "dGhlIHNhbXBsZSBub25jZQ==");
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
            let (mut stream, _) = listener.accept().await.unwrap();

            // create a buffer for the incoming handshake request
            let mut buffer: Vec<u8> = vec![0; 1024];
            let n = stream.read(&mut buffer).await.unwrap();

            let mut headers = [EMPTY_HEADER; 16];
            let mut req = Request::new(&mut headers);

            req.parse(&buffer[..n]).unwrap();

            // Already testing get_header_value func, since it's easier than spliting the string to
            // find sec_websocket_key
            let sec_websocket_key = req.get_header_value(SEC_WEBSOCKET_KEY).unwrap();

            let accept_key = generate_websocket_accept_value(sec_websocket_key);

            let response = HTTP_ACCEPT_RESPONSE.replace("{}", &accept_key);
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.flush().await.unwrap();
        });

        // Call the connect_async function for connecting to the server
        connect_async("ws://127.0.0.1:9005", None).await?;

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
            let mut client_connection = connect_async("ws://127.0.0.1:9006", None).await.unwrap();
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
}
