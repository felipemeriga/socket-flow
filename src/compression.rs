const PERMESSAGE_DEFLATE: &str = "permessage-deflate";
const CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";
const SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";
const CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";
const SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";


#[derive(Debug, Clone, Default)]
pub struct Extensions {
    pub permessage_deflate: bool,
    pub client_no_context_takeover: Option<bool>,
    pub server_no_context_takeover: Option<bool>,
    pub client_max_window_bits: Option<i32>,
    pub server_max_window_bits: Option<i32>,
}


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
                extensions.client_max_window_bits = extension_str.trim().split('=').last()?.parse::<i32>().ok();
            }
        } else if extension_str.trim().starts_with(SERVER_MAX_WINDOW_BITS) {
            if !extension_str.contains('=') {
                extensions.server_max_window_bits = Some(15);
            } else {
                extensions.server_max_window_bits = extension_str.trim().split('=').last()?.parse::<i32>().ok();
            }
        }
    }
    if !extensions.permessage_deflate {
        return None;
    }

    Some(extensions)
}