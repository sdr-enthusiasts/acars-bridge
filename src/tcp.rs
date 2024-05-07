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
use tokio::io::BufWriter;
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
        sender: Option<Sender<String>>,
        stats: Sender<u8>,
    ) -> Result<Self, Error> {
        let stream = match StubbornTcpStream::connect_with_options(
            format!("{host}:{port}"),
            reconnect_options(format!("{host}:{port}").as_str()),
        )
        .await
        {
            Ok(stream) => stream,
            Err(e) => {
                panic!("[TCP Input {host}:{port}] Error connecting {e}");
            }
        };

        // return self now
        Ok(Self {
            host: host.to_string(),
            port,
            socket: stream,
            sender,
            stats,
        })
    }

    async fn receive_message(self) {
        let name = self.format_name();
        let reader = tokio::io::BufReader::new(self.socket);
        let mut lines = Framed::new(reader, LinesCodec::new());

        while let Some(Ok(line)) = lines.next().await {
            debug!("{}Received: {}", name, line);

            if let Some(sender) = &self.sender {
                match sender.send(line.clone()).await {
                    Ok(()) => trace!("{}Message sent to output channel", name),
                    Err(e) => panic!("{name}Error sending message to output channel: {e}"),
                }
            }

            match self.stats.send(1).await {
                Ok(()) => trace!("{}Stats sent to channel", name),
                Err(e) => panic!("{name}Error sending stats to channel: {e}"),
            }
        }

        info!("{}Connection closed, shutting down", name);
    }

    fn format_name(&self) -> String {
        format!("[TCP Input {}:{}] ", self.host, self.port)
    }
}

#[async_trait]
impl OutputServer for OutputServerOptions<StubbornIo<TcpStream, String>> {
    async fn new(host: &str, port: u16, receiver: Receiver<String>) -> Result<Self, Error> {
        let stream: StubbornIo<TcpStream, String> = match StubbornTcpStream::connect_with_options(
            format!("{host}:{port}"),
            reconnect_options(format!("{host}:{port}").as_str()),
        )
        .await
        {
            Ok(stream) => stream,
            Err(e) => {
                panic!("[TCP Output {host}:{port}] Error connecting {e}");
            }
        };

        // return self now
        Ok(Self {
            host: host.to_string(),
            port,
            socket: stream,
            receiver,
        })
    }

    async fn watch_queue(mut self) {
        let name = self.format_name();
        let mut writer: BufWriter<StubbornIo<TcpStream, String>> = BufWriter::new(self.socket);
        while let Some(line) = self.receiver.recv().await {
            debug!("{}Received: {}", name, line);

            // verify we have a newline
            let line = if line.ends_with('\n') {
                line
            } else {
                format!("{line}\n")
            };

            match writer.write(line.as_bytes()).await {
                Ok(_) => {
                    debug!("{}Message sent to consumer", name);

                    match writer.flush().await {
                        Ok(()) => trace!("{}Flushed message to consumer", name),
                        Err(e) => {
                            panic!("{name}Error flushing message to consumer: {e}")
                        }
                    };
                }
                Err(e) => panic!("{name}Error sending message to consumer: {e}"),
            }
        }

        info!("{}Queue is empty, shutting down", name);
    }

    fn format_name(&self) -> String {
        format!("[TCP Output {}:{}] ", self.host, self.port)
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
