use crate::error::Error;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader, ReadHalf};
use tokio::time::{timeout, Duration};
use url::Url;
use crate::extensions::{add_extension_headers, Extensions};

const HTTP_REQUEST_DELIMITER: &str = "\r\n\r\n";

// Function used for client connection, parsing the ws/wss URL to http, for constructing the
// handshake request, which includes the sec-websockets-key, the URL path, scheme and another relevant
// info. This function also returns the hostname since this is necessary for establishing the TCP socket
pub fn construct_http_request(
    ws_url: &str,
    key: &str,
    extensions: Option<Extensions>
) -> Result<(String, String, String, bool), Error> {
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
        None => String::from(host),
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
    let mut request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n",
        request_path,
        request_host_field,
        key,
    );

    add_extension_headers(&mut request, extensions);

    Ok((request, host_with_port, String::from(host), use_tls))
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub async fn parse_http_request<T: AsyncReadExt + Unpin>(
        reader: &mut BufReader<ReadHalf<T>>,
    ) -> Result<HttpRequest, Error> {
        let mut buffer = String::new();

        // Adding a timeout to the buffer read, since some attackers may only connect to the TCP
        // endpoint, and froze without sending the HTTP handshake.
        // Therefore, we need to drop all these cases
        timeout(Duration::from_secs(5), async {
            // Read headers until we find the blank line (\r\n\r\n)
            while let Ok(bytes_read) = reader.read_line(&mut buffer).await {
                if bytes_read == 0 || buffer.ends_with(HTTP_REQUEST_DELIMITER) {
                    break;
                }
            }
        })
        .await?;

        // Split the headers from the body
        let (header_part, body_part) = match buffer.split_once("\r\n\r\n") {
            Some(parts) => parts,
            None => return Err(Error::HttpParseError),
        };

        // Parse the request line (e.g., "GET /path HTTP/1.1")
        let mut lines = header_part.lines();
        let request_line = lines.next().ok_or(Error::InvalidHTTPRequestLine)?;
        let mut parts = request_line.split_whitespace();
        let method = parts.next().ok_or(Error::MissingHTTPMethod)?.to_string();
        let uri = parts.next().ok_or(Error::MissingHTTPUri)?.to_string();
        let version = parts.next().ok_or(Error::MissingHTTPVersion)?.to_string();

        // Parse headers
        let mut headers = HashMap::new();
        for line in lines {
            if let Some((key, value)) = line.split_once(": ") {
                headers.insert(key.to_string().to_lowercase(), value.trim().to_string());
            }
        }

        // Read the body based on Content-Length
        let body = if let Some(content_length) = headers.get("Content-Length") {
            let length: usize = content_length
                .parse()
                .map_err(|_| Error::InvalidContentLength)?;
            let mut body_buf = vec![0; length];
            reader.read_exact(&mut body_buf).await?;
            body_buf
        } else {
            body_part.as_bytes().to_vec() // No Content-Length, use existing body part
        };

        Ok(HttpRequest {
            method,
            uri,
            version,
            headers,
            body,
        })
    }

    pub fn get_header_value(&mut self, key: &str) -> Option<String> {
        self.headers.get(key).cloned()
    }
}
