// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::Error;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::{Receiver, Sender};

pub enum SocketType {
    Tcp,
    Udp,
    Zmq,
}

impl From<String> for SocketType {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "tcp" => SocketType::Tcp,
            "udp" => SocketType::Udp,
            "zmq" => SocketType::Zmq,
            _ => panic!("Unknown Socket Type: {}", s),
        }
    }
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
    pub host: String,
    pub port: u16,
    pub socket: T,
    pub sender: Option<Sender<String>>,
    pub stats: Sender<u8>,
}

pub struct OutputServerOptions<T> {
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
        sender: Option<Sender<String>>,
        stats: Sender<u8>,
    ) -> Result<Self, Error>
    where
        Self: Sized;
    async fn receive_message(self);
    fn format_name(&self) -> String;
}

#[async_trait]
pub trait OutputServer {
    async fn new(host: &str, port: u16, receiver: Receiver<String>) -> Result<Self, Error>
    where
        Self: Sized;
    async fn watch_queue(self);
    fn format_name(&self) -> String;
}
