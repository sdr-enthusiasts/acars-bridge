use crate::serverconfig::InputServer;
use crate::serverconfig::InputServerOptions;
// Copyright (c) 2024-2026 Fred Clausen
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
                        if let Err(e) = sender.send(composed_message.to_string()).await {
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
                    trace!("{}Stats sent to stats channel", self.format_name());
                }
                Err(e) => {
                    // recv_from can surface transient kernel errors (e.g.
                    // ICMP-driven ECONNREFUSED from a prior send_to, EINTR)
                    // that don't warrant tearing down the bound socket. Log
                    // and keep reading; the supervisor would otherwise rebind
                    // for no reason and could even race with EADDRINUSE.
                    error!("{}recv_from error: {:?}", self.format_name(), e);
                }
            }
        }
    }

    fn format_name(&self) -> String {
        format!("[UDP Input {}] ", self.port)
    }
}

#[async_trait]
impl OutputServer for OutputServerOptions<UdpSocket> {
    async fn new(host: &str, port: u16) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0".to_string()).await?;
        Ok(Self {
            host: host.to_string(),
            port,
            socket,
        })
    }

    async fn watch_queue(self, receiver: &mut Receiver<String>) -> Result<(), Error> {
        let dest = format!("{}:{}", self.host, self.port);
        loop {
            match receiver.recv().await {
                Some(message) => {
                    debug!("{}Received: {}", self.format_name(), message);

                    // verify we have a newline
                    let message = if message.ends_with('\n') {
                        message
                    } else {
                        format!("{message}\n")
                    };

                    // Send the entire message as a single UDP datagram. The
                    // kernel handles IP fragmentation transparently for
                    // messages larger than the path MTU; the receiver's
                    // kernel reassembles before delivery. The previous
                    // application-level chunking produced multiple
                    // independent datagrams that the receiver had no way to
                    // recombine, silently corrupting any message > 8192
                    // bytes.
                    //
                    // The hard ceiling here is the UDP payload max
                    // (~65507 bytes); messages larger than that produce
                    // EMSGSIZE, which we log and drop.
                    let bytes = message.as_bytes();
                    match self.socket.send_to(bytes, &dest).await {
                        Ok(n) if n < bytes.len() => {
                            // Per POSIX, a UDP send_to either transmits the
                            // entire datagram or fails. A short return would
                            // indicate a kernel anomaly worth flagging.
                            warn!(
                                "{}Short UDP send: {} of {} bytes",
                                self.format_name(),
                                n,
                                bytes.len()
                            );
                        }
                        Ok(_) => trace!("{}Message sent to consumer", self.format_name()),
                        Err(e) => {
                            // UDP is best-effort; log and continue. Common
                            // causes: EMSGSIZE (oversized message),
                            // ENETUNREACH, ICMP-driven errors from a prior
                            // datagram.
                            error!(
                                "{}Error sending message ({} bytes) to consumer: {}",
                                self.format_name(),
                                bytes.len(),
                                e
                            );
                        }
                    }
                }
                None => {
                    return Err(Error::msg(format!(
                        "{}Input channel closed",
                        self.format_name()
                    )));
                }
            }
        }
    }

    fn format_name(&self) -> String {
        format!("[UDP Output {}:{}] ", self.host, self.port)
    }
}
