use futures::StreamExt;
use log::{error, info};
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, private_key};
use socket_flow::handshake::accept_async;
use socket_flow::stream::SocketFlowStream;
use std::fs::File;
use std::io::{self, BufReader, ErrorKind};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsStream};

async fn handle_connection(_: SocketAddr, stream: TlsStream<TcpStream>) {
    match accept_async(SocketFlowStream::Secure(stream)).await {
        Ok(mut ws_connection) => {
            while let Some(result) = ws_connection.next().await {
                match result {
                    Ok(message) => {
                        if ws_connection.send_message(message).await.is_err() {
                            error!("Failed to send message");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Received error from the stream: {}", e);
                        break;
                    }
                }
            }
        }
        Err(err) => error!("Error when performing handshake: {}", err),
    }
}
fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}

fn load_key(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    Ok(private_key(&mut BufReader::new(File::open(path)?))
        .unwrap()
        .ok_or(io::Error::new(
            ErrorKind::Other,
            "no private key found".to_string(),
        ))?)
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let addr = String::from("127.0.0.1:9002")
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::AddrNotAvailable))?;

    let certs = load_certs(Path::new("server.crt"))?;
    let key = load_key(Path::new("server.key"))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    let acceptor = TlsAcceptor::from(Arc::new(config));

    let listener = TcpListener::bind(&addr).await?;

    while let Ok((stream, peer)) = listener.accept().await {
        info!("Peer address: {}", peer);
        match acceptor.accept(stream).await {
            Ok(tls_stream) => {
                tokio::spawn(handle_connection(peer, TlsStream::Server(tls_stream)));
            }
            Err(err) => {
                error!("TLS handshake failed with {}: {}", peer, err);
            }
        }
    }

    Ok(())
}
