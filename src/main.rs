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
    clippy::all,
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
)]

#[macro_use]
extern crate log;

pub mod config;
pub mod serverconfig;
pub mod stats;

pub mod tcp;
pub mod udp;
pub mod zmq;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use sdre_rust_logging::SetupLogging;
use sdre_stubborn_io::tokio::StubbornIo;
use serverconfig::InputServerOptions;
use tmq::publish::Publish;
use tmq::subscribe::Subscribe;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

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

    // ----- Input server -----
    info!("Creating input server");
    let input_handle = spawn_input(&config, input, stats_input).await?;

    // ----- Optional output server -----
    let output_handle = spawn_output_if_configured(&config, output).await?;

    // Pipeline is linear: if any leg dies, the bridge is useless. Await
    // whichever task ends first and surface its outcome to the process
    // exit code.
    let outcome = match output_handle {
        Some(out) => tokio::select! {
            r = input_handle  => ("input",  r),
            r = out           => ("output", r),
        },
        None => ("input", input_handle.await),
    };

    match outcome {
        (which, Ok(Ok(()))) => {
            info!("{which} task ended cleanly; shutting down");
            Ok(())
        }
        (which, Ok(Err(e))) => Err(e).with_context(|| format!("{which} task failed")),
        (which, Err(join_err)) => {
            Err(anyhow!(join_err).context(format!("{which} task panicked or was cancelled")))
        }
    }
}

async fn spawn_input(
    config: &Config,
    input: Option<mpsc::Sender<String>>,
    stats_input: mpsc::Sender<u8>,
) -> Result<JoinHandle<Result<()>>> {
    let proto = SocketType::try_from(config.get_source_protocol())
        .with_context(|| format!("invalid source protocol {:?}", config.get_source_protocol()))?;

    let host = config.get_source_host();
    let port = config.get_source_port();

    Ok(match proto {
        SocketType::Tcp => {
            let server =
                InputServerOptions::<StubbornIo<TcpStream>>::new(host, port, input, stats_input)
                    .await?;
            tokio::spawn(async move { server.receive_message().await })
        }
        SocketType::Udp => {
            let server =
                InputServerOptions::<tokio::net::UdpSocket>::new(host, port, input, stats_input)
                    .await?;
            tokio::spawn(async move { server.receive_message().await })
        }
        SocketType::Zmq => {
            let server =
                InputServerOptions::<Subscribe>::new(host, port, input, stats_input).await?;
            tokio::spawn(async move { server.receive_message().await })
        }
    })
}

async fn spawn_output_if_configured(
    config: &Config,
    output_rx: Option<mpsc::Receiver<String>>,
) -> Result<Option<JoinHandle<Result<()>>>> {
    if !config.is_destination_set() {
        return Ok(None);
    }

    let output_rx = output_rx
        .ok_or_else(|| anyhow!("output channel was not created despite is_destination_set"))?;
    let host = config
        .get_destination_host()
        .clone()
        .ok_or_else(|| anyhow!("destination host missing despite is_destination_set"))?;
    let port = config
        .get_destination_port()
        .ok_or_else(|| anyhow!("destination port missing despite is_destination_set"))?;
    let proto = config
        .get_destination_protocol()
        .clone()
        .ok_or_else(|| anyhow!("destination protocol missing despite is_destination_set"))?;

    info!("Creating output server");

    let handle = match SocketType::try_from(proto.clone())
        .with_context(|| format!("invalid destination protocol {proto:?}"))?
    {
        SocketType::Tcp => {
            let server =
                OutputServerOptions::<StubbornIo<TcpStream>>::new(&host, port, output_rx).await?;
            tokio::spawn(async move { server.watch_queue().await })
        }
        SocketType::Udp => {
            let server =
                OutputServerOptions::<tokio::net::UdpSocket>::new(&host, port, output_rx).await?;
            tokio::spawn(async move { server.watch_queue().await })
        }
        SocketType::Zmq => {
            let server = OutputServerOptions::<Publish>::new(&host, port, output_rx).await?;
            tokio::spawn(async move { server.watch_queue().await })
        }
    };
    Ok(Some(handle))
}
