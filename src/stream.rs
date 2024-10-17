use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
#[cfg(feature = "feature-native-tls")]
use tokio_native_tls::TlsStream as NativeTlsStream;
use tokio_rustls::TlsStream as RustTlsStream;

// We need to implement AsyncRead and AsyncWrite for SocketFlowStream,
// because when we split a TlsStream, it returns a ReadHalf<T>, WriteHalf<T>
// where T: AsyncRead + AsyncWrite
// This is a good solution, when you don't want to use a generic for your own functions
// If we use a generic in accept_async(handshake.rs), we will need to add trait signatures to all the
// functions that are called inside accept_async recursively.
pub enum SocketFlowStream {
    Plain(TcpStream),
    Rustls(RustTlsStream<TcpStream>),
    #[cfg(feature = "feature-native-tls")]
    NativeTls(NativeTlsStream<TcpStream>),
}

impl AsyncRead for SocketFlowStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            SocketFlowStream::Plain(ref mut s) => Pin::new(s).poll_read(cx, buf),
            SocketFlowStream::Rustls(s) => Pin::new(s).poll_read(cx, buf),
            #[cfg(feature = "feature-native-tls")]
            SocketFlowStream::NativeTls(s) => Pin::new(s).poll_read(cx, buf),
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
            SocketFlowStream::Rustls(s) => Pin::new(s).poll_write(cx, buf),
            #[cfg(feature = "feature-native-tls")]
            SocketFlowStream::NativeTls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            SocketFlowStream::Plain(ref mut s) => Pin::new(s).poll_flush(cx),
            SocketFlowStream::Rustls(s) => Pin::new(s).poll_flush(cx),
            #[cfg(feature = "feature-native-tls")]
            SocketFlowStream::NativeTls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            SocketFlowStream::Plain(ref mut s) => Pin::new(s).poll_shutdown(cx),
            SocketFlowStream::Rustls(s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(feature = "feature-native-tls")]
            SocketFlowStream::NativeTls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
