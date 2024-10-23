use crate::config::ServerConfig;
use crate::event::{generate_new_uuid, Event, EventStream};
use crate::handshake::accept_async_with_config;
use crate::stream::SocketFlowStream;
use futures::StreamExt;
use std::io::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::{TlsAcceptor, TlsStream};

/// A ready to use websockets server
///
/// This method is used to spawn a websockets server with just several lines of code.
/// It accepts a port where the server will run, and the ServerConfig, which contains custom
/// websockets configurations, and a TLS config option; in case the end-user wants to enable
/// TLS on this server.
/// It returns an EventStream, which is a stream
/// that notifies all the relevant events of the websockets server, like new connected clients
/// messages from a single client, disconnections and errors.
pub async fn start_server_with_config(
    port: u16,
    config: Option<ServerConfig>,
) -> Result<EventStream, Error> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    let (tx, rx) = mpsc::channel(1000);
    let web_socket_config = config.clone().unwrap_or_default().web_socket_config;
    let tls_config = config.unwrap_or_default().tls_config;
    // This method will return an EventStream, which holds a Receiver channel. Therefore, this
    // spawned task will be used for processing new connections,
    // messages, disconnections and errors, concurrently.
    tokio::spawn(async move {
        loop {
            // we are using UUID, which is more flexible, and secure than incrementing IDs
            let uuid = generate_new_uuid();
            match listener.accept().await {
                Ok((stream, _)) => {
                    let socket_stream = if let Some(config) = tls_config.clone() {
                        let acceptor = TlsAcceptor::from(config);
                        match acceptor.accept(stream).await {
                            Ok(tls_stream) => SocketFlowStream::Secure(TlsStream::from(tls_stream)),
                            Err(err) => {
                                tx.send(Event::Error(uuid, err.into())).await.unwrap();
                                continue;
                            }
                        }
                    } else {
                        SocketFlowStream::Plain(stream)
                    };

                    let ws_connection =
                        match accept_async_with_config(socket_stream, web_socket_config.clone())
                            .await
                        {
                            Ok(conn) => conn,
                            Err(err) => {
                                tx.send(Event::Error(uuid, err)).await.unwrap();
                                continue;
                            }
                        };
                    // splitting the connection, so we could monitor incoming messages into a
                    // separate task, and handover the writer to the end-user
                    let (mut ws_reader, ws_writer) = ws_connection.split();

                    // send new client event
                    tx.send(Event::NewClient(uuid, ws_writer)).await.unwrap();

                    let tx_task = tx.clone();
                    tokio::spawn(async move {
                        while let Some(result) = ws_reader.next().await {
                            match result {
                                Ok(message) => {
                                    tx_task
                                        .send(Event::NewMessage(uuid, message))
                                        .await
                                        .unwrap();
                                    // send the received message event
                                }
                                Err(err) => {
                                    tx_task.send(Event::Error(uuid, err)).await.unwrap();
                                    break;
                                }
                            }
                        }

                        // send disconnect event when connection closed
                        let _ = tx_task.send(Event::Disconnect(uuid)).await;
                    });
                }
                Err(error) => {
                    tx.send(Event::Error(uuid, error.into())).await.unwrap();
                    continue;
                }
            }
        }
    });

    // Delivery the EventStream to the end-user, without blocking this function call
    // by the spawned task.
    // Thus, processing and sending new events concurrently
    Ok(EventStream::new(rx))
}

/// A ready to use websockets server
///
/// This method is used to spawn a websockets server with just several lines of code.
/// It accepts a port where the server will run, and returns an EventStream, which is a stream
/// that notifies all the relevant events of the websockets server, like new connected clients
/// messages from a single client, disconnections and errors.
pub async fn start_server(port: u16) -> Result<EventStream, Error> {
    start_server_with_config(port, None).await
}
