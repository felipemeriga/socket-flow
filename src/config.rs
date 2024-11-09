use rustls::ServerConfig as RustlsConfig;
use std::sync::Arc;
use crate::compression::Extensions;

/// Used for spawning a websockets server, including the general websocket
/// connection configuration, and a tls_config, which is basically a TLS config
/// in the case you want to have TLS enabled for your server.
#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    pub web_socket_config: Option<WebSocketConfig>,
    /// We currently support tokio-rustls/rustls for enabling TLS on server-side
    /// This config holds information about the TLS certificate-chain and everything
    /// that should be taken into consideration over the TLS setup.
    pub tls_config: Option<Arc<RustlsConfig>>,
}

/// Used for connecting over websocket endpoints as a client
/// including the general websocket connection configuration
/// with the addition of a ca_file, in the case the server you
/// are trying to connect uses a self-signed certificate.
#[derive(Debug, Clone, Default)]
pub struct ClientConfig {
    pub web_socket_config: WebSocketConfig,
    /// In some cases, the server you are trying to connect would use
    /// a self-signed certificate.
    /// On these cases, the client would not be able to verify the CA signature of that
    /// certificate,
    /// only by looking over our browser/operational system certificate store of trusted known CAs.
    /// The client would need to provide the CA file used to sign the server certificate
    /// otherwise the TLS handshake would fail.
    /// This TLS setup is mostly used for development,
    /// and we don't recommend for production purposes
    pub ca_file: Option<String>,
}

/// Stores general configurations, to replace some default websockets connection parameters
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Maximum value for Frame payload size, not counting the underlying basic frame components.
    /// By default, the maximum value is set as 16 MiB(Mebibyte) = 16 * 1024 * 1024
    /// Increasing it over these limits, may impact the performance of the application, as well as
    /// the security, since malicious user can constantly send huge Frames.
    pub max_frame_size: Option<usize>,
    /// A message may be compounded by multiple Frames.
    /// Therefore, this config variable denotes the
    /// maximum payload size a message can have.
    /// The default is 64 MiB, which is reasonably big.
    pub max_message_size: Option<usize>,
    /// This represents the extensions that will be applied, enabling compression and
    /// modifying relevant specs about server and client compression.
    pub extensions: Option<Extensions>
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        WebSocketConfig {
            max_message_size: Some(64 << 20),
            max_frame_size: Some(16 << 20),
            extensions: None,
        }
    }
}
