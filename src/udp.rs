use std::error::Error;

use crate::serverconfig::InputServer;
use crate::serverconfig::OutputServer;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{Receiver, Sender};

impl InputServer<UdpSocket> {
    pub async fn new(
        host: &str,
        port: u16,
        sender: Sender<String>,
        stats: Sender<u8>,
    ) -> Result<Self, Box<dyn Error>> {
        let socket = UdpSocket::bind(format!("{}:{}", host, port)).await?;
        Ok(InputServer {
            proto_name: "udp".to_string(),
            host: host.to_string(),
            port,
            socket,
            sender,
            stats,
        })
    }

    pub async fn receive_message(self) {
        let mut buf = [0; 8192];
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((size, _)) => {
                    let composed_message = String::from_utf8_lossy(&buf[..size]);
                    trace!(
                        "[UDP LISTENER SERVER {}] Received: {}",
                        self.proto_name,
                        composed_message
                    );

                    match self.sender.send(composed_message.to_string()).await {
                        Ok(_) => trace!(
                            "[UDP LISTENER SERVER {}] Message sent to channel",
                            self.proto_name
                        ),
                        Err(e) => error!(
                            "[UDP LISTENER SERVER {}] Error sending message to channel: {}",
                            self.proto_name, e
                        ),
                    }

                    match self.stats.send(1).await {
                        Ok(_) => trace!(
                            "[UDP LISTENER SERVER {}] Stats sent to channel",
                            self.proto_name
                        ),
                        Err(e) => error!(
                            "[UDP LISTENER SERVER {}] Error sending stats to channel: {}",
                            self.proto_name, e
                        ),
                    }
                }
                Err(e) => error!("[UDP LISTENER SERVER {}] Error: {:?}", self.proto_name, e),
            }
        }
    }
}

impl OutputServer<UdpSocket> {
    pub async fn new(
        host: &str,
        port: u16,
        receiver: Receiver<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let socket = UdpSocket::bind("0.0.0.0:0".to_string()).await?;
        Ok(OutputServer {
            proto_name: format!("UDP:{}:{}", host, port),
            host: host.to_string(),
            port,
            socket,
            receiver,
        })
    }

    pub async fn watch_queue(&mut self) {
        let max_size = 8192;
        loop {
            match self.receiver.recv().await {
                Some(message) => {
                    // convert string to bytes
                    let bytes = message.as_bytes();
                    // send bytes to destination. If the message is larger than the max size, send up to max size and keep sending until the entire message is sent.
                    let mut offset = 0;
                    while offset < bytes.len() {
                        let end = std::cmp::min(offset + max_size, bytes.len());
                        match self
                            .socket
                            .send_to(&bytes[offset..end], format!("{}:{}", self.host, self.port))
                            .await
                        {
                            Ok(_) => {
                                trace!(
                                    "[UDP SENDER SERVER {}] Sent: {}",
                                    self.proto_name,
                                    String::from_utf8_lossy(&bytes[offset..end])
                                );
                                offset = end;
                            }
                            Err(e) => {
                                error!(
                                    "[UDP SENDER SERVER {}] Error sending message: {}",
                                    self.proto_name, e
                                );
                                break;
                            }
                        }
                    }
                }
                None => {
                    error!(
                        "[UDP SENDER SERVER {}] Error receiving message from channel",
                        self.proto_name
                    );
                }
            }
        }
    }
}
