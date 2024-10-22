use rustls::ServerConfig as RustlsConfig;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    pub web_socket_config: Option<WebSocketConfig>,
    pub tls_config: Option<Arc<RustlsConfig>>,
}

#[derive(Debug, Clone, Default)]
pub struct ClientConfig {
    pub web_socket_config: WebSocketConfig,
    pub ca_file: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    pub max_frame_size: Option<usize>,
    pub max_message_size: Option<usize>,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        WebSocketConfig {
            max_message_size: Some(64 << 20),
            max_frame_size: Some(16 << 20),
        }
    }
}
