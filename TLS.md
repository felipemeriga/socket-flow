# TLS

## Introduction

This library supports TLS on clients/servers to provide secure encrypted TCP packets for websockets 
stream.
In this file, we will discuss how-to set it up using `socket-flow`, showing some server/client examples.

By default, this library only accepts [tokio-rustls](https://github.com/rustls/tokio-rustls), as an adapter library
for adding TLS in your client/server implementation with `socket-flow`.

## Self-Signed vs Trusted Certificate 

This library supports self-signed certificates for local development,
and trusted certificates for production environments.

Regardless of using self-signed certificates or trusted certificates, for creating websocket servers from 
this library, the setup would be the same.
Although, for setting up clients, if you are using a self-signed certificate on server, when calling `connect_async`
for establishing a client connection, you should add the argument `ca_file`, which resolves to the server certificate.

If you are using trusted certificates, connect using the client straightaway.

We will now present some examples and how to set up TLS.

## Server Example (Self-Signed)

Here is an echo-server TLS example that you can also find in:
[Example](https://github.com/felipemeriga/socket-flow/blob/main/examples/echo_server_tls.rs)

```rust
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
        .ok_or_else(|| io::Error::from(ErrorKind::AddrNotAvailable))?;

    let certs = load_certs(Path::new("cert.pem"))?;
    let key = load_key(Path::new("key.pem"))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(ErrorKind::InvalidInput, err))?;

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
```

For running this example, you will need a key and a certificate, and you can generate a self-signed one, like this:
```shell
openssl genpkey -algorithm RSA -out key.pem -pkeyopt rsa_keygen_bits:2048
```

```shell
openssl req -x509 -new -key key.pem -out cert.pem -days 365
```

Place them in the root of the cloned repo, and you can run this example by:
```shell
cargo run --color=always --package socket-flow --example echo_server_tls
```

## Server Example (Trusted Certificate)

If you are planning to use trusted certificates on the server,
you can use the same example for self-signed; the setup will be the same.

## Client Example (Trusted Certificate)

Assume the server you are trying to connect is a third-party server. Which already has a trusted certificate,
here is an example of code, where we are going to collect info about real-time cryptocurrency market data:

```rust
use futures::StreamExt;
use log::*;
use socket_flow::handshake::connect_async;

async fn handle_connection(addr: &str) {
    match connect_async(addr).await {
        Ok(mut ws_connection) => {
            while let Some(result) = ws_connection.next().await {
                match result {
                    Ok(message) => {
                        info!("Received message: {}", message.as_text().unwrap());
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

#[tokio::main]
async fn main() {
    env_logger::init();
    handle_connection("wss://api.gemini.com/v1/marketdata/BTCUSD").await;
}
```

## Server + Client Example (Self-Signed)

Like we mentioned in the beginning of this guide, if you are using self-signed certificates for a server, the client
should also have the certificate, because as the certificate is not trusted by a valid CA,
your client code won't be able to validate the server certificate unless you provide it directly to the client.

For this example, we will need to create some certificates,
first creating a file called: `server_cert_config.cnf`, adding the following content:
```
[ req ]
default_bits       = 2048
distinguished_name = req_distinguished_name
req_extensions     = req_ext
prompt             = no

[ req_distinguished_name ]
CN = localhost

[ req_ext ]
subjectAltName = @alt_names

[ alt_names ]
DNS.1 = localhost
IP.1 = 127.0.0.1
```

This file will be created to set some configs on the server certificate.
Now, for creating all the certificates, execute the following shell script:

```shell
# Step 1: Create the CA (Certificate Authority)
openssl genrsa -out ca.key 2048
openssl req -x509 -new -nodes -key ca.key -sha256 -days 365 -out ca.crt -subj "/CN=MyTestCA"

# Step 2: Create the server key and CSR
openssl genrsa -out server.key 2048
openssl req -new -key server.key -out server.csr -subj "/CN=localhost"

# Step 3: Use the cert config file for updating the server certificate
openssl req -new -key server.key -out server.csr -config server_cert_config.cnf

# Sign the server certificate with the CA, including SAN for both localhost and 127.0.0.1
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
-out server.crt -days 365 -sha256 -extfile server_cert_config.cnf -extensions req_ext
```

In case, you aren't creating a new shell script file, and pasting this script, execute line by line in the terminal.
Place all the created certificates, in the same place you are going to run the server/client code,
otherwise you should change in the following presented examples, where are your certificates located.

Now, you can run the server first:
```rust
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
        .ok_or_else(|| io::Error::from(ErrorKind::AddrNotAvailable))?;

    let certs = load_certs(Path::new("server.crt"))?;
    let key = load_key(Path::new("server.key"))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(ErrorKind::InvalidInput, err))?;

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
```

Finally, execute the client code:
```rust
use futures::StreamExt;
use log::*;
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use socket_flow::config::ClientConfig;
use socket_flow::handshake::connect_async_with_config;
use tokio::select;
use tokio::time::{interval, Duration};

async fn handle_connection(addr: &str) {
    let mut client_config = ClientConfig::default();
    client_config.ca_file = Some(String::from("ca.crt"));

    match connect_async_with_config(addr, Some(client_config)).await {
        Ok(mut ws_connection) => {
            let mut ticker = interval(Duration::from_secs(5));
            // it will be used for closing the connection
            let mut counter = 0;

            loop {
                select! {
                    Some(result) = ws_connection.next() => {
                        match result {
                            Ok(message) => {
                                 info!("Received message: {}", message.as_text().unwrap());
                                counter = counter + 1;
                                // close the connection if 3 messages have already been sent and received
                                if counter >= 3 {
                                    if ws_connection.close_connection().await.is_err() {
                                         error!("Error occurred when closing connection");
                                    }
                                    break;
                                }
                            }
                            Err(err) => {
                                error!("Received error from the stream: {}", err);

                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        let random_string = generate_random_string();
                        let binary_data = Vec::from(random_string);

                        if ws_connection.send(binary_data).await.is_err() {
                            eprintln!("Failed to send message");
                            break;
                        }
                    }
                }
            }
        }
        Err(err) => error!("Error when performing handshake: {}", err),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    handle_connection("wss://localhost:9002").await;
}

fn generate_random_string() -> String {
    rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}
```

You can check more examples over [Examples](https://github.com/felipemeriga/socket-flow/tree/main/examples)