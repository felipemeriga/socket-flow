const PERMESSAGE_DEFLATE: &str = "permessage-deflate";
const CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";
const SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";
const CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";
const SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";

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
                extensions.client_max_window_bits =
                    extension_str.trim().split('=').last()?.parse::<u8>().ok();
            }
        } else if extension_str.trim().starts_with(SERVER_MAX_WINDOW_BITS) {
            if !extension_str.contains('=') {
                extensions.server_max_window_bits = Some(15);
            } else {
                extensions.server_max_window_bits =
                    extension_str.trim().split('=').last()?.parse::<u8>().ok();
            }
        }
    }
    if !extensions.permessage_deflate {
        return None;
    }

    Some(extensions)
}

pub fn merge_extensions(
    server_extensions: Option<Extensions>,
    client_extensions: Option<Extensions>,
) -> Option<Extensions> {
    let server_ext = match server_extensions {
        Some(ext) => ext,
        None => return None,
    };
    let client_ext = match client_extensions {
        Some(ext) => ext,
        None => return None,
    };
    let merged_extensions = Extensions {
        permessage_deflate: client_ext.permessage_deflate && server_ext.permessage_deflate,
        client_no_context_takeover: server_ext
            .client_no_context_takeover
            .and(client_ext.client_no_context_takeover),
        server_no_context_takeover: server_ext
            .server_no_context_takeover
            .and(client_ext.server_no_context_takeover),
        client_max_window_bits: match (
            server_ext.client_max_window_bits,
            client_ext.client_max_window_bits,
        ) {
            (Some(server_bits), Some(client_bits)) => Some(std::cmp::min(server_bits, client_bits)),
            (Some(server_bits), None) => Some(server_bits),
            (None, Some(client_bits)) => Some(client_bits),
            (None, None) => None,
        },
        server_max_window_bits: match (
            server_ext.server_max_window_bits,
            client_ext.server_max_window_bits,
        ) {
            (Some(server_bits), Some(client_bits)) => Some(std::cmp::min(server_bits, client_bits)),
            (Some(server_bits), None) => Some(server_bits),
            (None, Some(client_bits)) => Some(client_bits),
            (None, None) => None,
        },
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
                if let Some(true) = extensions.client_no_context_takeover {
                    request.push_str(&format!("; {}", CLIENT_NO_CONTEXT_TAKEOVER))
                }
                if let Some(true) = extensions.server_no_context_takeover {
                    request.push_str(&format!("; {}", SERVER_NO_CONTEXT_TAKEOVER))
                }
                if let Some(bits) = extensions.client_max_window_bits {
                    request.push_str(&format!("; {}={}", CLIENT_MAX_WINDOW_BITS, bits))
                }
                if let Some(bits) = extensions.server_max_window_bits {
                    request.push_str(&format!("; {}={}", SERVER_MAX_WINDOW_BITS, bits))
                }
            }
            request.push_str("\r\n\r\n");
        }
    }
}
