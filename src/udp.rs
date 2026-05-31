use crate::serverconfig::InputServer;
use crate::serverconfig::InputServerOptions;
// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::{Context, Error, Result, anyhow};
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
        let socket = UdpSocket::bind(format!("{host}:{port}")).await?;
        Ok(Self {
            host: host.to_string(),
            port,
            socket,
            sender,
            stats,
        })
    }

    async fn receive_message(self) -> Result<(), Error> {
        let name = self.format_name();
        let mut buf = [0; 8192];
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((size, _)) => {
                    if size == 0 {
                        warn!("{name}Received empty message");
                        continue;
                    }

                    let composed_message = String::from_utf8_lossy(&buf[..size]);

                    debug!("{name}Received: {composed_message}");

                    if let Some(sender) = &self.sender {
                        sender
                            .send(composed_message.to_string())
                            .await
                            .with_context(|| {
                                format!("{name}output channel closed; downstream task is gone")
                            })?;
                        trace!("{name}Message sent to sender channel");
                    }

                    self.stats
                        .send(1)
                        .await
                        .with_context(|| format!("{name}stats channel closed"))?;
                    trace!("{name}Stats sent to stats channel");
                }
                Err(e) => error!("{name}Error: {e:?}"),
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
        Ok(Self {
            host: host.to_string(),
            port,
            socket,
            receiver,
        })
    }

    async fn watch_queue(mut self) -> Result<(), Error> {
        let name = self.format_name();
        let max_size = 8192;

        loop {
            let Some(message) = self.receiver.recv().await else {
                // Sender side of the input channel was dropped: the upstream
                // task has exited, so the pipeline is broken. Surface as an
                // error so main brings the whole process down.
                return Err(anyhow!("{name}input channel closed; upstream task is gone"));
            };

            debug!("{name}Received: {message}");

            // convert string to bytes
            // verify we have a newline
            let message = if message.ends_with('\n') {
                message
            } else {
                format!("{message}\n")
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
                        trace!("{name}Message sent to consumer");
                        offset = end;
                    }
                    Err(e) => {
                        // Other sender types treat this as fatal, but UDP is a
                        // best-effort protocol so we just log and continue.
                        error!("{name}Error sending message to consumer: {e}");
                        // Skip the rest of this message; if the network is
                        // down we'll find out again on the next iteration.
                        break;
                    }
                }
            }
        }
    }

    fn format_name(&self) -> String {
        format!("[UDP Output {}:{}] ", self.host, self.port)
    }
}
