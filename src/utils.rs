use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use rand::random;
use sha1::{Digest, Sha1};

pub(crate) const UUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub(crate) fn generate_websocket_accept_value(key: String) -> String {
    let mut sha1 = Sha1::new();
    sha1.update(key.as_bytes());
    sha1.update(UUID.as_bytes());
    BASE64_STANDARD.encode(sha1.finalize())
}

pub(crate) fn generate_websocket_key() -> String {
    let random_bytes: [u8; 16] = random();
    BASE64_STANDARD.encode(random_bytes)
}
