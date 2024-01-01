#[macro_use]
extern crate log;

pub mod config;
pub mod serverconfig;
pub mod stats;

pub mod tcp;
pub mod udp;

use clap::Parser;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::serverconfig::{InputServer, OutputServer, SocketType};
use sdre_rust_logging::SetupLogging;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
            unimplemented!("TCP not implemented yet")
        }
        SocketType::Udp => {
            let input_server = InputServer::<tokio::net::UdpSocket>::new(
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

    match config.get_destination_protocol().into() {
        SocketType::Tcp => {
            unimplemented!("TCP not implemented yet")
        }
        SocketType::Udp => {
            let mut output_server = OutputServer::<tokio::net::UdpSocket>::new(
                config.get_destination_host(),
                config.get_destination_port(),
                output,
            )
            .await?;

            tokio::spawn(async move {
                output_server.watch_queue().await;
            });
        }
        SocketType::Zmq => {
            unimplemented!("ZMQ not implemented yet")
        }
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}
