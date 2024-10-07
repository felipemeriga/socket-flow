use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;

// We need to implement AsyncRead and AsyncWrite for SocketFlowStream,
// because when we split a TlsStream, it returns a ReadHalf<T>, WriteHalf<T>
// where T: AsyncRead + AsyncWrite
// This is a good solution, when you don't want to use a generic for your own functions
// If we use a generic in accept_async(handshake.rs), we will need to add trait signatures to all the
// functions that are called inside accept_async recursively.
pub enum SocketFlowStream {
    Plain(TcpStream),
    Secure(TlsStream<TcpStream>),
}

impl AsyncRead for SocketFlowStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            SocketFlowStream::Plain(ref mut s) => Pin::new(s).poll_read(cx, buf),
            SocketFlowStream::Secure(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for SocketFlowStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            SocketFlowStream::Plain(ref mut s) => Pin::new(s).poll_write(cx, buf),
            SocketFlowStream::Secure(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            SocketFlowStream::Plain(ref mut s) => Pin::new(s).poll_flush(cx),
            SocketFlowStream::Secure(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            SocketFlowStream::Plain(ref mut s) => Pin::new(s).poll_shutdown(cx),
            SocketFlowStream::Secure(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
