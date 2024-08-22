use crate::error::Error;
use crate::message::Message;
use crate::split::WSWriter;
use futures::Stream;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;

pub type ID = Uuid;

// Used for generating a new UUID, every time a new client connects the server
pub fn generate_new_uuid() -> Uuid {
    let mut rng = StdRng::from_rng(rand::thread_rng());
    let buf = rng.random::<[u8; 16]>();

    Uuid::new_v8(buf)
}

// Base enum, used as the structure to represent every single event within
// the websockets server, offering the end-user a practical way of spawning a server
// and handling connections
pub enum Event {
    NewClient(ID, WSWriter),
    NewMessage(ID, Message),
    Disconnect(ID),
    Error(ID, Error),
}

// This struct will be used for implementing Stream trait. Thus, the end-user
// doesn't need to interact with the mpsc tokio channel directly
pub struct EventStream {
    receiver: Receiver<Event>,
}

impl EventStream {
    pub fn new(receiver: Receiver<Event>) -> Self {
        Self { receiver }
    }
}

impl Stream for EventStream {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.receiver).poll_recv(cx)
    }
}
