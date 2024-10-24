use futures::StreamExt;
use log::*;
use socket_flow::event::{Event, ID};
use socket_flow::server::start_server;
use socket_flow::split::WSWriter;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    env_logger::init();

    let port: u16 = 8080;
    match start_server(port).await {
        Ok(mut event_receiver) => {
            let mut clients: HashMap<ID, WSWriter> = HashMap::new();
            info!("Server started on address 127.0.0.1:{}", port);
            while let Some(event) = event_receiver.next().await {
                match event {
                    Event::NewClient(id, client_conn) => {
                        info!("New client {} connected", id);
                        clients.insert(id, client_conn);
                    }
                    Event::NewMessage(client_id, message) => {
                        info!("Message from client {}: {:?}", client_id, message);
                        let ws_writer = clients.get_mut(&client_id).unwrap();
                        ws_writer.send_message(message).await.unwrap();
                    }
                    Event::Disconnect(client_id) => {
                        info!("Client {} disconnected", client_id);
                        clients.remove(&client_id);
                    }
                    Event::Error(client_id, error) => {
                        error!("Error occurred for client {}: {:?}", client_id, error);
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("Could not start the server due to: {:?}", err);
        }
    }
}
