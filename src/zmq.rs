// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::{Context as _, Error, Result};
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use tmq::Context;
use tmq::publish;
use tmq::publish::Publish;
use tmq::subscribe;
use tmq::subscribe::Subscribe;
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

    async fn receive_message(mut self) -> Result<(), Error> {
        let name = self.format_name();
        while let Some(msg) = self.socket.next().await {
            let message = match msg {
                Ok(message) => message,
                Err(e) => {
                    error!("{name}Error: {e:?}");
                    continue;
                }
            };

            let composed_message = message
                .iter()
                .map(|item| item.as_str().unwrap_or("invalid text"))
                .collect::<Vec<&str>>()
                .join(" ");

            debug!("{name}Received: {composed_message}");
            let stripped = composed_message
                .strip_suffix("\r\n")
                .or_else(|| composed_message.strip_suffix('\n'))
                .unwrap_or(&composed_message);

            if let Some(sender) = &self.sender {
                sender.send(stripped.to_string()).await.with_context(|| {
                    format!("{name}output channel closed; downstream task is gone")
                })?;
                trace!("{name}Message sent to sender channel");
            }

            self.stats
                .send(1)
                .await
                .with_context(|| format!("{name}stats channel closed"))?;
            trace!("{name}Stats sent to channel");
        }

        info!("{name}Subscribe socket stream ended, shutting down");
        Ok(())
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

    async fn watch_queue(mut self) -> Result<(), Error> {
        let name = self.format_name();
        while let Some(message) = self.receiver.recv().await {
            debug!("{name}Received: {message}");

            let message_zmq = vec![&message];

            self.socket
                .send(message_zmq)
                .await
                .with_context(|| format!("{name}publish to consumer failed"))?;
            trace!("{name}Message sent to consumer");
        }

        info!("{name}Input channel closed, shutting down");
        Ok(())
    }

    fn format_name(&self) -> String {
        format!("[ZMQ Output {}:{}] ", self.host, self.port)
    }
}
