use tokio::io::{BufReader, BufWriter};
use tokio::net::TcpStream;

pub struct WebsocketsStream<R, W> {
    pub read: BufReader<R>,
    pub write: BufWriter<W>,
}