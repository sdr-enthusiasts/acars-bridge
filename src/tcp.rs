// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use anyhow::{Error, Result};
use async_trait::async_trait;
use sdre_stubborn_io::config::DurationIterator;
use sdre_stubborn_io::tokio::StubbornIo;
use sdre_stubborn_io::ReconnectOptions;
use sdre_stubborn_io::StubbornTcpStream;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use crate::serverconfig::InputServer;
use crate::serverconfig::InputServerOptions;
use crate::serverconfig::OutputServer;
use crate::serverconfig::OutputServerOptions;

#[async_trait]
impl InputServer for InputServerOptions<StubbornIo<TcpStream, String>> {
    async fn new(
        host: &str,
        port: u16,
        sender: Sender<String>,
        stats: Sender<u8>,
    ) -> Result<Self, Error> {
        let stream = match StubbornTcpStream::connect_with_options(
            format!("{}:{}", host, port),
            reconnect_options(format!("{}:{}", host, port).as_str()),
        )
        .await
        {
            Ok(stream) => stream,
            Err(e) => {
                error!(
                    "[TCP Receiver Server {}:{}] Error connecting {}",
                    host, port, e
                );
                Err(e)?
            }
        };

        // return self now
        Ok(InputServerOptions {
            proto_name: "tcp".to_string(),
            host: host.to_string(),
            port,
            socket: stream,
            sender,
            stats,
        })
    }

    async fn receive_message(self) {
        let reader = tokio::io::BufReader::new(self.socket);
        let mut lines = Framed::new(reader, LinesCodec::new());
        while let Some(Ok(line)) = lines.next().await {
            trace!(
                "[TCP LISTENER SERVER {}] Received: {}",
                self.proto_name,
                line
            );

            match self.sender.send(line).await {
                Ok(_) => trace!(
                    "[TCP LISTENER SERVER {}] Message sent to channel",
                    self.proto_name
                ),
                Err(e) => error!(
                    "[TCP LISTENER SERVER {}] Error sending message to channel: {}",
                    self.proto_name, e
                ),
            }

            match self.stats.send(1).await {
                Ok(_) => trace!(
                    "[TCP LISTENER SERVER {}] Stats sent to channel",
                    self.proto_name
                ),
                Err(e) => error!(
                    "[TCP LISTENER SERVER {}] Error sending stats to channel: {}",
                    self.proto_name, e
                ),
            }
        }
    }
}

#[async_trait]
impl OutputServer for OutputServerOptions<StubbornIo<TcpStream, String>> {
    async fn new(host: &str, port: u16, receiver: Receiver<String>) -> Result<Self, Error> {
        let stream = match StubbornTcpStream::connect_with_options(
            format!("{}:{}", host, port),
            reconnect_options(format!("{}:{}", host, port).as_str()),
        )
        .await
        {
            Ok(stream) => stream,
            Err(e) => {
                error!(
                    "[TCP Sender Server {}:{}] Error connecting {}",
                    host, port, e
                );
                Err(e)?
            }
        };

        // return self now
        Ok(OutputServerOptions {
            proto_name: "tcp".to_string(),
            host: host.to_string(),
            port,
            socket: stream,
            receiver,
        })
    }

    async fn watch_queue(mut self) {
        let mut writer = tokio::io::BufWriter::new(self.socket);
        while let Some(line) = self.receiver.recv().await {
            trace!("[TCP SENDER SERVER {}] Received: {}", self.proto_name, line);

            // verify we have a newline
            let line = if line.ends_with('\n') {
                line
            } else {
                format!("{}\n", line)
            };

            match writer.write_all(line.as_bytes()).await {
                Ok(_) => trace!(
                    "[TCP SENDER SERVER {}] Message sent to channel",
                    self.proto_name
                ),
                Err(e) => error!(
                    "[TCP SENDER SERVER {}] Error sending message to channel: {}",
                    self.proto_name, e
                ),
            }
        }
    }
}

pub fn reconnect_options(host: &str) -> ReconnectOptions {
    ReconnectOptions::new()
        .with_exit_if_first_connect_fails(false)
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
