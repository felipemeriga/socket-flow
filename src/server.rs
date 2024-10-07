use crate::event::{generate_new_uuid, Event, EventStream};
use crate::handshake::accept_async;
use crate::stream::SocketFlowStream;
use futures::StreamExt;
use std::io::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// A ready to use websockets server
///
/// This method is used to spawn a websockets server with just several lines of code.
/// Accepts as argument that port, where the server will be running, and returns an `EventStream`.
/// Which implements Stream trait, being capable of processing a stream of events sequentially
/// notifying the end-user, about new client connections, disconnections, messages and errors.
pub async fn start_server(port: u16) -> Result<EventStream, Error> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    let (tx, rx) = mpsc::channel(1000);
    // This method will return an EventStream, which holds a Receiver channel. Therefore, this
    // spawned task will be used for processing new connections,
    // messages, disconnections and errors, concurrently.
    tokio::spawn(async move {
        loop {
            // we are using UUID, which is more flexible, and secure than incrementing IDs
            let uuid = generate_new_uuid();
            match listener.accept().await {
                Ok((stream, _)) => {
                    let ws_connection = match accept_async(SocketFlowStream::Plain(stream)).await {
                        Ok(conn) => conn,
                        Err(err) => {
                            tx.send(Event::Error(uuid, err)).await.unwrap();
                            break;
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
                    break;
                }
            }
        }
    });

    // Delivery the EventStream to the end-user, without blocking this function call
    // by the spawned task.
    // Thus, processing and sending new events concurrently
    Ok(EventStream::new(rx))
}
