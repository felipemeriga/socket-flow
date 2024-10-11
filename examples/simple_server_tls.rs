use futures::StreamExt;
use log::{error, info};
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, private_key};
use socket_flow::event::{Event, ID};
use socket_flow::server::start_server;
use rustls::ServerConfig;
use socket_flow::split::WSWriter;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufReader, ErrorKind};
use std::path::Path;
use std::sync::Arc;

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

async fn run_server(port: u16, config: Arc<ServerConfig>) {
    match start_server(8080, Some(config)).await {
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

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let certs = load_certs(Path::new("cert.pem"))?;
    let key = load_key(Path::new("key.pem"))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    let port: u16 = 8080;

    run_server(port, Arc::new(config)).await;

    Ok(())
}
