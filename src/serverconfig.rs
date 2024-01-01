use std::error::Error;

use async_trait::async_trait;
use tokio::sync::mpsc::{Receiver, Sender};

pub enum SocketType {
    Tcp,
    Udp,
    Zmq,
}

impl From<&str> for SocketType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "tcp" => SocketType::Tcp,
            "udp" => SocketType::Udp,
            "zmq" => SocketType::Zmq,
            _ => panic!("Unknown Socket Type: {}", s),
        }
    }
}

impl From<&String> for SocketType {
    fn from(s: &String) -> Self {
        match s.to_lowercase().as_str() {
            "tcp" => SocketType::Tcp,
            "udp" => SocketType::Udp,
            "zmq" => SocketType::Zmq,
            _ => panic!("Unknown Socket Type: {}", s),
        }
    }
}

pub struct InputServerOptions<T> {
    pub proto_name: String,
    pub host: String,
    pub port: u16,
    pub socket: T,
    pub sender: Sender<String>,
    pub stats: Sender<u8>,
}

pub struct OutputServerOptions<T> {
    pub proto_name: String,
    pub host: String,
    pub port: u16,
    pub socket: T,
    pub receiver: Receiver<String>,
}

#[async_trait]
pub trait InputServer {
    async fn new(
        host: &str,
        port: u16,
        sender: Sender<String>,
        stats: Sender<u8>,
    ) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
    async fn receive_message(self);
}

#[async_trait]
pub trait OutputServer {
    async fn new(host: &str, port: u16, receiver: Receiver<String>) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
    async fn watch_queue(self);
}
