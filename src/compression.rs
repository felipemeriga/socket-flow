use flate2::{Decompress, FlushDecompress, Status};
use crate::compression::MaxWindowBits::{Eight, Eleven, Fifteen, Fourteen, Nine, Ten, Thirteen, Twelve};
use crate::error::Error;

const PERMESSAGE_DEFLATE: &str = "permessage-deflate";
const CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";
const SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";
const CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";
const SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";

#[derive(Clone)]
pub enum MaxWindowBits {
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Eleven = 11,
    Twelve = 12,
    Thirteen = 13,
    Fourteen = 14,
    Fifteen = 15,
}

impl MaxWindowBits {
    fn from_u8(value: u8) -> Result<Self, Error> {
        match value {
            8 => Ok(Eight),
            9 => Ok(Nine),
            10 => Ok(Ten),
            11 => Ok(Eleven),
            12 => Ok(Twelve),
            13 => Ok(Thirteen),
            14 => Ok(Fourteen),
            15 => Ok(Fifteen),
            _ => Err(Error::InvalidMaxWindowBits)
        }
    }

    fn to_u8(&self) -> u8 {
       self.to_u8()
    }
}


/// It's important to enhance that some compression extensions,
/// in some cases affects compression and
/// decompression(client_no_context_takeover, server_no_context_takeover),
/// while another one affects only compression(client_max_window_bits, server_max_window_bits).
/// Keeping the context between compression and decompression,
/// improves performance but adds more overhead, consuming more memory.
/// Larger window sizes (closer to 15)
/// result in better compression ratios but are slower and use more memory.
/// Smaller window sizes (closer to 8) offer faster performance but with worse compression.
#[derive(Debug, Clone, Default)]
pub struct Extensions {
    /// Dictates if compression is enabled
    pub permessage_deflate: bool,
    /// Asks that the client should reset its compression context after compressing a message,
    /// if accepted by the server,
    /// the server must reset the compression context when decompressing each message.
    /// Bear in mind
    /// that this option is related to resetting the context when the client compresses,
    /// and when the server decompresses.
    /// The opposite is not valid.
    pub client_no_context_takeover: Option<bool>,
    /// Asks that the server should reset its compression context after compressing a message,
    /// if a client asks this, and the server accepts,
    /// the client must reset the compression context when decompressing each message.
    /// Bear in mind
    /// that this option is related to resetting the context when the server compresses,
    /// and when the client decompresses.
    /// The opposite is not valid.
    pub server_no_context_takeover: Option<bool>,
    /// Asks that the client sets its compression window to a specific number.
    pub client_max_window_bits: Option<u8>,
    /// Asks that the server sets its compression window to a specific number.
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

pub fn merge_extensions(server_extensions: Option<Extensions>, client_extensions: Option<Extensions>) -> Option<Extensions> {
    let server_ext = match server_extensions {
        Some(ext) => ext,
        None => return None,
    };
    let client_ext = match client_extensions {
        Some(ext) => ext,
        None => return None,
    };
    let mut merged_extensions =  Extensions::default();
    merged_extensions.permessage_deflate =  client_ext.permessage_deflate && server_ext.permessage_deflate;
    merged_extensions.client_no_context_takeover = server_ext.client_no_context_takeover.and(client_ext.client_no_context_takeover);
    merged_extensions.server_no_context_takeover = server_ext.server_no_context_takeover.and(client_ext.server_no_context_takeover);

    merged_extensions.client_max_window_bits = match (server_ext.client_max_window_bits, client_ext.client_max_window_bits) {
        (Some(server_bits), Some(client_bits)) => Some(std::cmp::min(server_bits, client_bits)),
        (Some(server_bits), None) => Some(server_bits),
        (None, Some(client_bits)) => Some(client_bits),
        (None, None) => Some(8),
    };

    merged_extensions.server_max_window_bits = match (server_ext.server_max_window_bits, client_ext.server_max_window_bits) {
        (Some(server_bits), Some(client_bits)) => Some(std::cmp::min(server_bits, client_bits)),
        (Some(server_bits), None) => Some(server_bits),
        (None, Some(client_bits)) => Some(client_bits),
        (None, None) => Some(8),
    };
    Some(merged_extensions)
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
    pub fn new() -> Self {
        let decompressor = Decompress::new(false);

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