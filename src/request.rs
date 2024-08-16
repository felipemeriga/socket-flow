use crate::error::Error;
use url::Url;

// Function used for client connection, parsing the ws/wss URL to http, for constructing the
// handshake request, which includes the sec-websockets-key, the URL path, scheme and another relevant
// info. This function also returns the hostname since this is necessary for establishing the TCP socket
pub fn parse_to_http_request(ws_url: &str, key: &str) -> Result<(String, String), Error> {
    let parsed_url = Url::parse(ws_url)?;

    let http_scheme = match parsed_url.scheme() {
        "ws" => "http",
        "wss" => "https",
        _ => return Err(Error::InvalidSchemeURL),
    };

    let host_with_port = match parsed_url.port() {
        Some(port) => format!(
            "{}:{}",
            parsed_url.host_str().ok_or(Error::URLNoHost)?,
            port
        ),
        None => return Err(Error::URLNoPort),
    };

    let request_path = match parsed_url.query() {
        Some(query) => format!("{}?{}", parsed_url.path(), query),
        None => parsed_url.path().to_string(),
    };

    // Since we already have all the info, it isn't worth converting everything to a HTTP request type
    // and considering everything is bits into the TCP packets, we simply manipulate the string, and
    // convert it to bytes when sending to the server
    let request = format!(
        "GET {} {}/1.1\r\nHost: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Extensions: permessage-deflate; client_max_window_bits\r\n\r\n",
        request_path,
        http_scheme.to_uppercase(),
        host_with_port,
        key,
    );

    Ok((request, host_with_port))
}
