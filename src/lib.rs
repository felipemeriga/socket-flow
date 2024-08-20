//! Simple async WebSockets implementation for Tokio stack.
//!
//! This library is supposed to offer a simple implementation for websockets, so end-user could use this to wrap a
//! websockets server/client into their application, offering a smooth way of setting it up into his code.
//!
//! It's an async library based on tokio runtime,
//! which uses a tokio TcpStream behind the scenes, using that as the starting point
//! to implement the standards of [WebSocket Protocol RFC](https://datatracker.ietf.org/doc/html/rfc6455),
//! performing handshakes, reading frames, parsing masks, handling opcodes and internal payload.
//!

mod connection;
pub mod error;
pub mod frame;
pub mod handshake;
mod read;
mod request;
mod write;
