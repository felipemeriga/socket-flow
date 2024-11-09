use flate2::{Decompress, FlushDecompress, Status};

const PERMESSAGE_DEFLATE: &str = "permessage-deflate";
const CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";
const SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";
const CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";
const SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";
const MAX_WINDOW_BITS_DEFAULT: u8 = 15;


#[derive(Debug, Clone, Default)]
pub struct Extensions {
    pub permessage_deflate: bool,
    pub client_no_context_takeover: Option<bool>,
    pub server_no_context_takeover: Option<bool>,
    pub client_max_window_bits: Option<u8>,
    pub server_max_window_bits: Option<u8>,
}

// TODO - In first instance we will use this function for server only
// In first stage server will accept all the client extension configs, and
// will reply the handshake request with everything that came from client
// on a second stage, the end-user will set the default extension settings when calling
// accept_async_with_config, and the server will read the client settings from the handshake
// and will merge with the default settings, prioritizing what is default
pub fn parse_extensions(extensions_header_value: String) -> Option<Extensions> {
    let extensions_str = extensions_header_value.split(';');
    let mut extensions = Extensions::default();

    for extension_str in extensions_str.into_iter() {
        if extension_str.trim() == PERMESSAGE_DEFLATE {
            extensions.permessage_deflate = true;
        } else if extension_str.trim().starts_with(CLIENT_NO_CONTEXT_TAKEOVER) {
            extensions.client_no_context_takeover = Some(true);
        } else if extension_str.trim().starts_with(SERVER_NO_CONTEXT_TAKEOVER) {
            extensions.server_no_context_takeover = Some(true);
        } else if extension_str.trim().starts_with(CLIENT_MAX_WINDOW_BITS) {
            if !extension_str.contains('=') {
                extensions.client_max_window_bits = Some(15);
            } else {
                extensions.client_max_window_bits = extension_str.trim().split('=').last()?.parse::<u8>().ok();
            }
        } else if extension_str.trim().starts_with(SERVER_MAX_WINDOW_BITS) {
            if !extension_str.contains('=') {
                extensions.server_max_window_bits = Some(15);
            } else {
                extensions.server_max_window_bits = extension_str.trim().split('=').last()?.parse::<u8>().ok();
            }
        }
    }
    if !extensions.permessage_deflate {
        return None;
    }

    Some(extensions)
}

pub fn add_extension_headers(request: &mut String, extensions: Option<Extensions>) {
    match extensions {
        None => {
            request.push_str("\r\n");
        }
        Some(extensions) => {
            if extensions.permessage_deflate {
                request.push_str(&format!("Sec-WebSocket-Extensions: {}", PERMESSAGE_DEFLATE));
                if let Some(true) = extensions.client_no_context_takeover { request.push_str(&format!("; {}", CLIENT_NO_CONTEXT_TAKEOVER)) }
                if let Some(true) = extensions.server_no_context_takeover { request.push_str(&format!("; {}", SERVER_NO_CONTEXT_TAKEOVER)) }
                if let Some(bits) = extensions.client_max_window_bits { request.push_str(&format!("; {}={}", CLIENT_MAX_WINDOW_BITS, bits)) }
                if let Some(bits) = extensions.server_max_window_bits { request.push_str(&format!("; {}={}", SERVER_MAX_WINDOW_BITS, bits)) }
            }
            request.push_str("\r\n\r\n");
        }
    }
}


pub struct Decoder {
    pub decompressor: Decompress,
}

impl Decoder {
    // By default, a decompression process only considers the max_window_bits
    // retaining the same context for all the messages
    pub fn new(mut max_window_bits: Option<u8>) -> Self {
        let m_window_bits = max_window_bits.unwrap_or(MAX_WINDOW_BITS_DEFAULT);
        let decompressor = Decompress::new_with_window_bits(false, m_window_bits);

        Self { decompressor }
    }

    pub fn decompress(&mut self, payload: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut decompressed_data = Vec::with_capacity(payload.len() * 2);
        let mut buffer = vec![0u8; 131072]; // Buffer to read decompressed chunks into
        let mut offset = 0;

        // Reset the decompressor before decompression to ensure no leftover state
        self.decompressor.reset(false);

        while offset < payload.len() {
            // Slice the remaining payload to decompress in chunks
            let input = &payload[offset..];

            // Decompress data in chunks into the buffer
            let status = self.decompressor.decompress(input, &mut buffer, FlushDecompress::Sync)?;

            // Only extend with actual data, trimming the trailing zeros
            let bytes_written = self.decompressor.total_out() as usize - decompressed_data.len();
            decompressed_data.extend_from_slice(&buffer[..bytes_written]);

            // Calculate how much was read and adjust offset
            let bytes_consumed = self.decompressor.total_in() as usize - offset;
            offset += bytes_consumed;

            // Break if the decompression is done
            if let Status::StreamEnd = status {
                break;
            }
        }

        Ok(decompressed_data)
    }
}