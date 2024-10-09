use crate::error::Error;
use url::Url;

// Function used for client connection, parsing the ws/wss URL to http, for constructing the
// handshake request, which includes the sec-websockets-key, the URL path, scheme and another relevant
// info. This function also returns the hostname since this is necessary for establishing the TCP socket
pub fn parse_to_http_request(ws_url: &str, key: &str) -> Result<(String, String, String, bool), Error> {
    let parsed_url = Url::parse(ws_url)?;
    let mut use_tls = false;

    // Clause just to validate the user has passed the proper URL scheme
    // Also, we need to get the proper HTTP port for that, in the case ws_url
    // is a domain instead of an IP
    let http_port: u16 = match parsed_url.scheme() {
        "ws" => 80,
        "wss" => {
            use_tls = true;
            443
        }
        _ => return Err(Error::InvalidSchemeURL),
    };

    let host = parsed_url.host_str().ok_or(Error::URLNoHost)?;
    // In the case ws_url is a domain instead of an IP, we need the HTTP port for using in the
    // TCP connection string
    let port = parsed_url.port().unwrap_or(http_port);

    // This will be used in the handshake request.
    // If ws_url is an IP, we need to add the respective port
    // If ws_url is a DNS, we don't need the port
    let request_host_field = match parsed_url.port() {
        Some(port) => format!("{}:{}", host, port),
        None => String::from(host)
    };

    // We need the port together with the host for establishing a TCP connection
    // regardless ws_url is an IP or domain
    let host_with_port = format!("{}:{}", host, port);

    let request_path = match parsed_url.query() {
        Some(query) => format!("{}?{}", parsed_url.path(), query),
        None => parsed_url.path().to_string(),
    };

    // Since we already have all the info, it isn't worth converting everything to a HTTP request type
    // and considering everything is bits into the TCP packets, we simply manipulate the string, and
    // convert it to bytes when sending to the server
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        request_path,
        request_host_field,
        key,
    );

    Ok((request, host_with_port, String::from(host), use_tls))
}

pub trait RequestExt {
    fn get_header_value(&self, header_name: &str) -> Option<String>;
}

impl<'a, 'b> RequestExt for httparse::Request<'a, 'b> {
    fn get_header_value(&self, header_name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(header_name))
            .map(|header| String::from_utf8_lossy(header.value).to_string())
    }
}
