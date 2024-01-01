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

pub struct InputServer<T> {
    pub proto_name: String,
    pub host: String,
    pub port: u16,
    pub socket: T,
    pub sender: Sender<String>,
    pub stats: Sender<u8>,
}

pub struct OutputServer<T> {
    pub proto_name: String,
    pub host: String,
    pub port: u16,
    pub socket: T,
    pub receiver: Receiver<String>,
}
