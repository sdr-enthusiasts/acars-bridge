// Copyright (c) 2024-2026 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::{Context, Error, Result, anyhow};
use async_trait::async_trait;
use sdre_stubborn_io::ReconnectOptions;
use sdre_stubborn_io::StubbornTcpStream;
use sdre_stubborn_io::config::DurationIterator;
use sdre_stubborn_io::tokio::StubbornIo;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use tokio::net::TcpStream;
use tokio::net::lookup_host;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use crate::serverconfig::InputServer;
use crate::serverconfig::InputServerOptions;
use crate::serverconfig::OutputServer;
use crate::serverconfig::OutputServerOptions;

/// Resolve a `host:port` pair into the first available `SocketAddr`.
///
/// `sdre-stubborn-io` 0.7 narrowed `StubbornTcpStream` to take a `SocketAddr`
/// only, pushing DNS responsibility onto the caller. We resolve once at
/// connect time; if the destination's DNS changes during a long-running
/// session the cached `SocketAddr` will be stale. Pre-resolving is the
/// "bare `SocketAddr`" path explicitly called out as acceptable in the
/// crate's 0.7.1 migration notes.
///
/// Pass `(host, port)` as a tuple rather than formatting `"{host}:{port}"`
/// so that bare IPv6 literals (`2001:db8::1`) don't need to be bracketed
/// by the caller; the tuple form delegates parsing to tokio/std and
/// handles DNS names, IPv4, and IPv6 uniformly.
async fn resolve_host(host: &str, port: u16) -> Result<SocketAddr, Error> {
    lookup_host((host, port))
        .await
        .with_context(|| format!("DNS lookup failed for {host}:{port}"))?
        .next()
        .ok_or_else(|| anyhow!("No addresses resolved for {host}:{port}"))
}

#[async_trait]
impl InputServer for InputServerOptions<StubbornIo<TcpStream>> {
    async fn new(
        host: &str,
        port: u16,
        sender: Option<Sender<String>>,
        stats: Sender<u8>,
    ) -> Result<Self, Error> {
        let addr = resolve_host(host, port)
            .await
            .with_context(|| format!("[TCP Input {host}:{port}] DNS resolution failed"))?;

        let stream = StubbornTcpStream::connect_with_options(
            addr,
            reconnect_options(&format!("{host}:{port}")),
        )
        .await
        .map_err(|e| Error::msg(format!("[TCP Input {host}:{port}] Error connecting: {e}")))?;

        // return self now
        Ok(Self {
            host: host.to_string(),
            port,
            socket: stream,
            sender,
            stats,
        })
    }

    async fn receive_message(self) -> Result<(), Error> {
        let name = self.format_name();
        let reader = tokio::io::BufReader::new(self.socket);
        let mut lines = Framed::new(reader, LinesCodec::new());

        while let Some(result) = lines.next().await {
            let line = match result {
                Ok(l) => l,
                Err(e) => {
                    warn!("{name}Decode error, skipping line: {e}");
                    continue;
                }
            };

            debug!("{name}Received: {line}");

            if let Some(sender) = &self.sender {
                if let Err(e) = sender.send(line.clone()).await {
                    return Err(Error::msg(format!("{name}Output channel closed: {e}")));
                }
                trace!("{name}Message sent to output channel");
            }

            if let Err(e) = self.stats.send(1).await {
                return Err(Error::msg(format!("{name}Stats channel closed: {e}")));
            }
            trace!("{name}Stats sent to channel");
        }

        info!("{name}Connection closed by peer, shutting down");
        Ok(())
    }

    fn format_name(&self) -> String {
        format!("[TCP Input {}:{}] ", self.host, self.port)
    }
}

#[async_trait]
impl OutputServer for OutputServerOptions<StubbornIo<TcpStream>> {
    async fn new(host: &str, port: u16) -> Result<Self, Error> {
        let addr = resolve_host(host, port)
            .await
            .with_context(|| format!("[TCP Output {host}:{port}] DNS resolution failed"))?;

        let stream: StubbornIo<TcpStream> = StubbornTcpStream::connect_with_options(
            addr,
            reconnect_options(&format!("{host}:{port}")),
        )
        .await
        .map_err(|e| Error::msg(format!("[TCP Output {host}:{port}] Error connecting: {e}")))?;

        // return self now
        Ok(Self {
            host: host.to_string(),
            port,
            socket: stream,
        })
    }

    async fn watch_queue(self, receiver: &mut Receiver<String>) -> Result<(), Error> {
        let name = self.format_name();
        let mut writer: BufWriter<StubbornIo<TcpStream>> = BufWriter::new(self.socket);
        while let Some(line) = receiver.recv().await {
            debug!("{name}Received: {line}");

            // verify we have a newline
            let line = if line.ends_with('\n') {
                line
            } else {
                format!("{line}\n")
            };

            if let Err(e) = writer.write_all(line.as_bytes()).await {
                return Err(Error::msg(format!(
                    "{name}Error sending message to consumer: {e}"
                )));
            }
            debug!("{name}Message sent to consumer");

            if let Err(e) = writer.flush().await {
                return Err(Error::msg(format!(
                    "{name}Error flushing message to consumer: {e}"
                )));
            }
            trace!("{name}Flushed message to consumer");
        }

        // recv() returning None means all bridge Senders have been dropped,
        // which happens only during graceful shutdown (main drops the master
        // Sender after the input supervisor exits). Return Ok(()) so the
        // output supervisor treats this as a terminal, clean exit rather than
        // a failure to restart/log at error level.
        info!("{name}Input channel closed (shutdown); exiting");
        Ok(())
    }

    fn format_name(&self) -> String {
        format!("[TCP Output {}:{}] ", self.host, self.port)
    }
}

pub fn reconnect_options(host: &str) -> ReconnectOptions {
    // `with_exit_if_first_connect_fails(false)` is the default in 0.7 and has
    // been dropped from this builder chain accordingly.
    ReconnectOptions::new()
        .with_retries_generator(get_our_standard_reconnect_strategy)
        .with_connection_name(host)
}

fn get_our_standard_reconnect_strategy() -> DurationIterator {
    let initial_attempts = vec![
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(10),
        Duration::from_secs(20),
        Duration::from_secs(30),
        Duration::from_secs(40),
        Duration::from_secs(50),
        Duration::from_secs(60),
    ];

    let repeat = std::iter::repeat(Duration::from_secs(60));

    let forever_iterator = initial_attempts.into_iter().chain(repeat);

    Box::new(forever_iterator)
}
