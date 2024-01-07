use crate::serverconfig::InputServer;
use crate::serverconfig::InputServerOptions;
// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::{Error, Result};
use async_trait::async_trait;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::serverconfig::OutputServer;
use crate::serverconfig::OutputServerOptions;

#[async_trait]
impl InputServer for InputServerOptions<UdpSocket> {
    async fn new(
        host: &str,
        port: u16,
        sender: Option<Sender<String>>,
        stats: Sender<u8>,
    ) -> Result<Self, Error> {
        let socket = UdpSocket::bind(format!("{}:{}", host, port)).await?;
        Ok(InputServerOptions {
            host: host.to_string(),
            port,
            socket,
            sender,
            stats,
        })
    }

    async fn receive_message(self) {
        let mut buf = [0; 8192];
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((size, _)) => {
                    if size == 0 {
                        warn!("{}Received empty message", self.format_name());
                        continue;
                    }

                    let composed_message = String::from_utf8_lossy(&buf[..size]);

                    debug!("{}Received: {}", self.format_name(), composed_message);

                    if let Some(sender) = &self.sender {
                        match sender.send(composed_message.to_string()).await {
                            Ok(_) => trace!("{}Message sent to sender channel", self.format_name()),
                            Err(e) => panic!(
                                "{}Error sending message to sender channel: {}",
                                self.format_name(),
                                e
                            ),
                        }
                    }

                    match self.stats.send(1).await {
                        Ok(_) => trace!("{}Stats sent to stats channel", self.format_name()),
                        Err(e) => panic!(
                            "{}Error sending to stats channel: {}",
                            self.format_name(),
                            e
                        ),
                    }
                }
                Err(e) => error!("{}Error: {:?}", self.format_name(), e),
            }
        }
    }

    fn format_name(&self) -> String {
        format!("[UDP Input {}] ", self.port)
    }
}

#[async_trait]
impl OutputServer for OutputServerOptions<UdpSocket> {
    async fn new(host: &str, port: u16, receiver: Receiver<String>) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0".to_string()).await?;
        Ok(OutputServerOptions {
            host: host.to_string(),
            port,
            socket,
            receiver,
        })
    }

    async fn watch_queue(mut self) {
        let max_size = 8192;

        loop {
            match self.receiver.recv().await {
                Some(message) => {
                    debug!("{}Received: {}", self.format_name(), message);

                    // convert string to bytes
                    // verify we have a newline
                    let message = if message.ends_with('\n') {
                        message
                    } else {
                        format!("{}\n", message)
                    };
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
                                trace!("{}Message sent to consumer", self.format_name(),);
                                offset = end;
                            }
                            Err(e) => {
                                // Other sender types panic at this point, but UDP is a best effort protocol, so we just log the error and continue

                                error!(
                                    "{}Error sending message to consumer: {}",
                                    self.format_name(),
                                    e
                                );
                            }
                        }
                    }
                }
                None => {
                    panic!(
                        "{}Error receiving message from sender input channel",
                        self.format_name()
                    );
                }
            }

            info!("{}Queue is empty, shutting down", self.format_name());
        }
    }

    fn format_name(&self) -> String {
        format!("[UDP Output {}:{}] ", self.host, self.port)
    }
}
