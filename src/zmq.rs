// Copyright (c) 2024-2026 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::{Error, Result};
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use std::sync::OnceLock;
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

/// Return the process-wide `ZeroMQ` `Context`, creating it on first use.
///
/// `ZeroMQ` contexts spin up their own I/O threads and own all socket file
/// descriptors for the lifetime of the context. They are designed to be
/// long-lived singletons; creating one per socket (which is what happened
/// before this lookup, on every supervisor restart) leaks I/O threads and
/// FDs over time when restarts happen frequently.
fn zmq_context() -> &'static Context {
    static CONTEXT: OnceLock<Context> = OnceLock::new();
    CONTEXT.get_or_init(Context::new)
}

#[async_trait]
impl InputServer for InputServerOptions<Subscribe> {
    async fn new(
        host: &str,
        port: u16,
        sender: Option<Sender<String>>,
        stats: Sender<u8>,
    ) -> Result<Self, Error> {
        let address = format!("tcp://{host}:{port}");
        let socket = subscribe(zmq_context()).connect(&address)?.subscribe(b"")?;

        Ok(Self {
            host: host.to_string(),
            port,
            socket,
            sender,
            stats,
        })
    }

    async fn receive_message(mut self) -> Result<(), Error> {
        while let Some(msg) = self.socket.next().await {
            let message = match msg {
                Ok(message) => message,
                Err(e) => {
                    error!("{}Error: {:?}", self.format_name(), e);
                    continue;
                }
            };

            // Join all frames with spaces. ACARS payloads are 7-bit ASCII in
            // practice, but warn loudly if a frame contains non-UTF-8 bytes
            // so silent corruption is visible. Fall back to lossy conversion
            // so the bridge still forwards something.
            let composed_message = message
                .iter()
                .map(|item| match std::str::from_utf8(item) {
                    Ok(s) => std::borrow::Cow::Borrowed(s),
                    Err(e) => {
                        warn!(
                            "{}Non-UTF-8 frame ({} bytes, error at byte {}); using lossy conversion",
                            self.format_name(),
                            item.len(),
                            e.valid_up_to()
                        );
                        String::from_utf8_lossy(item)
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            debug!("{}Received: {}", self.format_name(), composed_message);
            let stripped = composed_message
                .strip_suffix("\r\n")
                .or_else(|| composed_message.strip_suffix('\n'))
                .unwrap_or(&composed_message);

            if let Some(sender) = &self.sender {
                if let Err(e) = sender.send(stripped.to_string()).await {
                    return Err(Error::msg(format!(
                        "{}Output channel closed: {}",
                        self.format_name(),
                        e
                    )));
                }
                trace!("{}Message sent to sender channel", self.format_name());
            }

            if let Err(e) = self.stats.send(1).await {
                return Err(Error::msg(format!(
                    "{}Stats channel closed: {}",
                    self.format_name(),
                    e
                )));
            }
            trace!("{}Stats sent to channel", self.format_name());
        }

        // A tmq Subscribe stream should not normally end on its own. If we
        // get here, something unexpected happened (context dropped, etc.).
        // Surface as Err so the supervisor logs at error! level rather than
        // info! and treats it as an abnormal restart.
        Err(Error::msg(format!(
            "{}ZMQ subscribe stream ended unexpectedly",
            self.format_name()
        )))
    }

    fn format_name(&self) -> String {
        format!("[ZMQ Input {}:{}] ", self.host, self.port)
    }
}

#[async_trait]
impl OutputServer for OutputServerOptions<Publish> {
    async fn new(host: &str, port: u16) -> Result<Self, Error> {
        let address = format!("tcp://{host}:{port}");
        let socket = publish(zmq_context()).connect(&address)?;

        Ok(Self {
            host: host.to_string(),
            port,
            socket,
        })
    }

    async fn watch_queue(mut self, receiver: &mut Receiver<String>) -> Result<(), Error> {
        while let Some(message) = receiver.recv().await {
            debug!("{}Received: {}", self.format_name(), message);

            let message_zmq = vec![&message];

            if let Err(e) = self.socket.send(message_zmq).await {
                return Err(Error::msg(format!(
                    "{}Error sending message to consumer: {}",
                    self.format_name(),
                    e
                )));
            }
            trace!("{}Message sent to consumer", self.format_name());
        }

        Err(Error::msg(format!(
            "{}Input channel closed",
            self.format_name()
        )))
    }

    fn format_name(&self) -> String {
        format!("[ZMQ Output {}:{}] ", self.host, self.port)
    }
}
