// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

#![deny(
    clippy::pedantic,
    //clippy::cargo,
    clippy::nursery,
    clippy::style,
    clippy::correctness,
    clippy::all
)]

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
use tmq::publish::Publish;
use tmq::subscribe::Subscribe;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::serverconfig::{InputServer, OutputServer, OutputServerOptions, SocketType};

#[tokio::main]
async fn main() -> Result<()> {
    let config: Config = Config::parse();
    config.get_log_level().enable_logging();
    config.show_config();

    let mut input = None;
    let output = if config.is_destination_set() {
        info!("Destination set, creating output channel");
        let (input_sender, input_receiver) = mpsc::channel(32);
        input = Some(input_sender);
        Some(input_receiver)
    } else {
        None
    };

    // Create the stats channel

    let (stats_input, stats_output) = mpsc::channel(32);
    let stats = stats::Stats::new(stats_output);

    let print_interval = config.get_stat_interval();

    tokio::spawn(async move {
        stats.run(print_interval);
    });
    // create the input server

    info!("Creating input server");
    match SocketType::try_from(config.get_source_protocol()) {
        Ok(SocketType::Tcp) => {
            let input_server = InputServerOptions::<StubbornIo<TcpStream, String>>::new(
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
        Ok(SocketType::Udp) => {
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
        Ok(SocketType::Zmq) => {
            let input_server = InputServerOptions::<Subscribe>::new(
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
        Err(e) => {
            panic!("Error creating input server: {e}");
        }
    }

    // create the output server

    if config.is_destination_set() {
        let output = output.unwrap_or_else(|| panic!("Output channel not created"));

        let host = config.get_destination_host().clone().unwrap();
        let port = config.get_destination_port().unwrap();
        let proto = config.get_destination_protocol().clone().unwrap();

        info!("Creating output server");

        match SocketType::try_from(proto) {
            Ok(SocketType::Tcp) => {
                let output_server =
                    OutputServerOptions::<StubbornIo<TcpStream, String>>::new(&host, port, output)
                        .await?;

                tokio::spawn(async move {
                    output_server.watch_queue().await;
                });
            }
            Ok(SocketType::Udp) => {
                let output_server =
                    OutputServerOptions::<tokio::net::UdpSocket>::new(&host, port, output).await?;

                tokio::spawn(async move {
                    output_server.watch_queue().await;
                });
            }
            Ok(SocketType::Zmq) => {
                let output_server =
                    OutputServerOptions::<Publish>::new(&host, port, output).await?;

                tokio::spawn(async move {
                    output_server.watch_queue().await;
                });
            }

            Err(e) => {
                panic!("Error creating output server: {e}");
            }
        }
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}
