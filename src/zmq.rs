// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::{Error, Result};
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use tmq::publish;
use tmq::publish::Publish;
use tmq::subscribe;
use tmq::subscribe::Subscribe;
use tmq::Context;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::serverconfig::InputServer;
use crate::serverconfig::InputServerOptions;
use crate::serverconfig::OutputServer;
use crate::serverconfig::OutputServerOptions;

#[async_trait]
impl InputServer for InputServerOptions<Subscribe> {
    async fn new(
        host: &str,
        port: u16,
        sender: Option<Sender<String>>,
        stats: Sender<u8>,
    ) -> Result<Self, Error> {
        let address = format!("tcp://{host}:{port}");
        let socket = subscribe(&Context::new())
            .connect(&address)?
            .subscribe(b"")?;

        Ok(Self {
            host: host.to_string(),
            port,
            socket,
            sender,
            stats,
        })
    }

    async fn receive_message(mut self) {
        while let Some(msg) = self.socket.next().await {
            let message = match msg {
                Ok(message) => message,
                Err(e) => {
                    error!("{}Error: {:?}", self.format_name(), e);
                    continue;
                }
            };

            let composed_message = message
                .iter()
                .map(|item| item.as_str().unwrap_or("invalid text"))
                .collect::<Vec<&str>>()
                .join(" ");

            debug!("{}Received: {}", self.format_name(), composed_message);
            let stripped = composed_message
                .strip_suffix("\r\n")
                .or_else(|| composed_message.strip_suffix('\n'))
                .unwrap_or(&composed_message);

            if let Some(sender) = &self.sender {
                match sender.send(stripped.to_string()).await {
                    Ok(()) => trace!("{}Message sent to sender channel", self.format_name()),
                    Err(e) => panic!(
                        "{}Error sending message to sender channel: {}",
                        self.format_name(),
                        e
                    ),
                }
            }

            match self.stats.send(1).await {
                Ok(()) => trace!("{}Stats sent to channel", self.format_name()),
                Err(e) => panic!(
                    "{}Error sending stats to channel: {}",
                    self.format_name(),
                    e
                ),
            }
        }
    }

    fn format_name(&self) -> String {
        format!("[ZMQ Input {}:{}] ", self.host, self.port)
    }
}

#[async_trait]
impl OutputServer for OutputServerOptions<Publish> {
    async fn new(host: &str, port: u16, receiver: Receiver<String>) -> Result<Self, Error> {
        let address = format!("tcp://{host}:{port}");
        let socket = publish(&Context::new()).connect(&address)?;

        Ok(Self {
            host: host.to_string(),
            port,
            socket,
            receiver,
        })
    }

    async fn watch_queue(mut self) {
        while let Some(message) = self.receiver.recv().await {
            debug!("{}Received: {}", self.format_name(), message);

            let message_zmq = vec![&message];

            match self.socket.send(message_zmq).await {
                Ok(()) => trace!("{}Message sent to consumer", self.format_name()),
                Err(e) => panic!(
                    "{}Error sending message to consumer: {}",
                    self.format_name(),
                    e
                ),
            }
        }
    }

    fn format_name(&self) -> String {
        format!("[ZMQ Output {}:{}] ", self.host, self.port)
    }
}
