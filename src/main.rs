// Copyright (c) 2024-2026 Fred Clausen
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
use std::time::Duration;
use tmq::publish::Publish;
use tmq::subscribe::Subscribe;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

use crate::config::Config;
use crate::serverconfig::{InputServer, OutputServer, OutputServerOptions, SocketType};

/// Spawn a supervised input server. `output_sender` is the master Sender clone
/// for the input->output bridge channel (None when no output is configured).
/// `stats_sender` is the master Sender clone for the stats channel. Both are
/// cloned for every (re)spawn so the master copies in main keep the channels
/// alive even when the input task dies.
///
/// Backoff: 1s, 2s, 4s, 8s, 16s, 32s, then capped at 60s. Resets to 1s after
/// the task survives for at least 60s, so a transient failure doesn't keep us
/// in the slow-retry regime forever.
fn spawn_input(
    proto: SocketType,
    host: String,
    port: u16,
    output_sender: Option<Sender<String>>,
    stats_sender: Sender<u8>,
) {
    let label = format!("input/{host}:{port}");
    tokio::spawn(async move {
        let mut backoff_secs: u64 = 1;
        loop {
            let started = tokio::time::Instant::now();
            let output_sender = output_sender.clone();
            let stats_sender = stats_sender.clone();
            let result: Result<()> = async {
                match &proto {
                    SocketType::Tcp => {
                        let server = InputServerOptions::<StubbornIo<TcpStream>>::new(
                            &host,
                            port,
                            output_sender,
                            stats_sender,
                        )
                        .await?;
                        server.receive_message().await?;
                        Ok(())
                    }
                    SocketType::Udp => {
                        let server = InputServerOptions::<tokio::net::UdpSocket>::new(
                            &host,
                            port,
                            output_sender,
                            stats_sender,
                        )
                        .await?;
                        server.receive_message().await?;
                        Ok(())
                    }
                    SocketType::Zmq => {
                        let server = InputServerOptions::<Subscribe>::new(
                            &host,
                            port,
                            output_sender,
                            stats_sender,
                        )
                        .await?;
                        server.receive_message().await?;
                        Ok(())
                    }
                }
            }
            .await;

            match result {
                Ok(()) => info!("[SUPERVISOR][{label}] Task exited gracefully; restarting"),
                Err(e) => error!("[SUPERVISOR][{label}] Task failed: {e}; restarting"),
            }

            if started.elapsed() >= Duration::from_secs(60) {
                backoff_secs = 1;
            }

            info!("[SUPERVISOR][{label}] Sleeping {backoff_secs}s before restart");
            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs.saturating_mul(2)).min(60);
        }
    });
}

/// Spawn a supervised output server. The output `Receiver<String>` is owned by
/// the supervisor and borrowed mutably into `watch_queue` for each restart.
/// Because the master `Sender<String>` lives in main, the receiver never sees
/// a `None` from a closed channel even if the input task dies.
fn spawn_output(proto: SocketType, host: String, port: u16, mut receiver: mpsc::Receiver<String>) {
    let label = format!("output/{host}:{port}");
    tokio::spawn(async move {
        let mut backoff_secs: u64 = 1;
        loop {
            let started = tokio::time::Instant::now();
            let result: Result<()> = async {
                match &proto {
                    SocketType::Tcp => {
                        let server =
                            OutputServerOptions::<StubbornIo<TcpStream>>::new(&host, port).await?;
                        server.watch_queue(&mut receiver).await?;
                        Ok(())
                    }
                    SocketType::Udp => {
                        let server =
                            OutputServerOptions::<tokio::net::UdpSocket>::new(&host, port).await?;
                        server.watch_queue(&mut receiver).await?;
                        Ok(())
                    }
                    SocketType::Zmq => {
                        let server = OutputServerOptions::<Publish>::new(&host, port).await?;
                        server.watch_queue(&mut receiver).await?;
                        Ok(())
                    }
                }
            }
            .await;

            match result {
                Ok(()) => info!("[SUPERVISOR][{label}] Task exited gracefully; restarting"),
                Err(e) => error!("[SUPERVISOR][{label}] Task failed: {e}; restarting"),
            }

            if started.elapsed() >= Duration::from_secs(60) {
                backoff_secs = 1;
            }

            info!("[SUPERVISOR][{label}] Sleeping {backoff_secs}s before restart");
            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs.saturating_mul(2)).min(60);
        }
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    let config: Config = Config::parse();
    config.get_log_level().enable_logging();
    config.show_config();

    let channel_capacity = config.get_channel_capacity();

    // Master bridge channel (input -> output). We retain the master Sender in
    // main so that even if all input tasks die simultaneously, the output side
    // does not see a closed channel.
    let (bridge_sender_master, bridge_receiver) = if config.is_destination_set() {
        info!("Destination set, creating output channel");
        let (tx, rx) = mpsc::channel::<String>(channel_capacity);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // Master stats channel. Same reasoning: the master Sender stays in main so
    // the stats receiver loop never observes a closed channel due to a dead
    // input task.
    let (stats_sender_master, stats_receiver) = mpsc::channel::<u8>(channel_capacity);

    let stats = stats::Stats::new(stats_receiver);
    let print_interval = config.get_stat_interval();
    stats.run(print_interval);

    // Spawn the supervised input.
    info!("Creating input server");
    let input_proto = SocketType::try_from(config.get_source_protocol())
        .map_err(|e| anyhow::anyhow!("Error parsing source protocol: {e}"))?;
    spawn_input(
        input_proto,
        config.get_source_host().to_string(),
        config.get_source_port(),
        bridge_sender_master.clone(),
        stats_sender_master.clone(),
    );

    // Spawn the supervised output, if configured.
    if config.is_destination_set() {
        let rx = bridge_receiver.expect("bridge receiver should exist when destination is set");
        let host = config
            .get_destination_host()
            .clone()
            .expect("destination host should be set");
        let port = config
            .get_destination_port()
            .expect("destination port should be set");
        let proto_str = config
            .get_destination_protocol()
            .as_deref()
            .expect("destination protocol should be set");
        let output_proto = SocketType::try_from(proto_str)
            .map_err(|e| anyhow::anyhow!("Error parsing destination protocol: {e}"))?;

        info!("Creating output server");
        spawn_output(output_proto, host, port, rx);
    }

    // Keep master Senders alive for the lifetime of the process by holding
    // them here. Without these bindings the compiler would drop them at the
    // end of `main`'s setup and close the channels we just worked to keep
    // open.
    let _bridge_keepalive = bridge_sender_master;
    let _stats_keepalive = stats_sender_master;

    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
