#[cfg(test)]
mod tests {
    use crate::frame::{Frame, OpCode};
    use crate::request::parse_to_http_request;

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
        let (request, host_with_port) = parse_to_http_request("ws://localhost:8080", "dGhlIHNhbXBsZSBub25jZQ==").unwrap();
        assert_eq!(host_with_port, "localhost:8080");
        assert!(request.starts_with("GET / HTTP/1.1"));
        assert!(request.contains("Host: localhost:8080"));
        assert!(request.contains("Upgrade: websocket"));
        assert!(request.contains("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ=="));
    }

    #[test]
    fn test_parse_to_http_request_no_port() {
        let result = parse_to_http_request("ws://localhost", "dGhlIHNhbXBsZSBub25jZQ==");
        assert!(result.is_err());
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
}