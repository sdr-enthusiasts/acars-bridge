// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

#[macro_use]
extern crate log;

pub mod config;
pub mod serverconfig;
pub mod stats;

pub mod tcp;
pub mod udp;
pub mod zmq;

use anyhow::Result;
use clap::Parser;
use sdre_rust_logging::SetupLogging;
use sdre_stubborn_io::tokio::StubbornIo;
use serverconfig::InputServerOptions;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::serverconfig::{InputServer, OutputServer, OutputServerOptions, SocketType};

#[tokio::main]
async fn main() -> Result<()> {
    let config: Config = Config::parse();
    config.get_log_level().enable_logging();
    config.show_config();

    // Create the input channel all receivers will send their data to.
    let (input, output) = mpsc::channel(32);

    // Create the stats channel

    let (stats_input, stats_output) = mpsc::channel(32);
    let stats = stats::Stats::new(stats_output);

    let print_interval = config.get_stat_interval();

    tokio::spawn(async move {
        stats.run(print_interval).await;
    });
    // create the input server

    match config.get_source_protocol().into() {
        SocketType::Tcp => {
            let input_server = InputServerOptions::<StubbornIo<TcpStream, SocketAddr>>::new(
                config.get_source_host(),
                config.get_source_port(),
                input,
                stats_input,
            )
            .await?;

            tokio::spawn(async move {
                input_server.receive_message().await;
            });
        }
        SocketType::Udp => {
            let input_server = InputServerOptions::<tokio::net::UdpSocket>::new(
                config.get_source_host(),
                config.get_source_port(),
                input,
                stats_input,
            )
            .await?;

            tokio::spawn(async move {
                input_server.receive_message().await;
            });
        }
        SocketType::Zmq => {
            unimplemented!("ZMQ not implemented yet")
        }
    }

    // create the output server

    if let Some(host) = config.get_destination_host() {
        if let Some(port) = config.get_destination_port() {
            if let Some(proto) = config.get_destination_protocol() {
                info!("Creating output server");
                match proto.into() {
                    SocketType::Tcp => {
                        let output_server =
                            OutputServerOptions::<StubbornIo<TcpStream, SocketAddr>>::new(
                                host, port, output,
                            )
                            .await?;

                        tokio::spawn(async move {
                            output_server.watch_queue().await;
                        });
                    }
                    SocketType::Udp => {
                        let output_server =
                            OutputServerOptions::<tokio::net::UdpSocket>::new(host, port, output)
                                .await?;

                        tokio::spawn(async move {
                            output_server.watch_queue().await;
                        });
                    }
                    SocketType::Zmq => {
                        unimplemented!("ZMQ not implemented yet")
                    }
                }
            }
        }
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}
