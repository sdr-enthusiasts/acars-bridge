// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.
pub extern crate clap as clap;
extern crate sdre_rust_logging;
use clap::Parser;

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "ACARS Bridge", author, version, about, long_about = None)]
pub struct Config {
    #[clap(long, env = "AB_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    #[clap(long, env = "AB_SOURCE", required = true, requires_all = &["source_port", "source_protocol"])]
    pub source_host: String,

    #[clap(long, env = "AB_SOURCE_PORT")]
    pub source_port: u16,

    #[clap(long, env = "AB_SOURCE_PROTOCOL")]
    pub source_protocol: String,

    #[clap(long, env = "AB_DESTINATION", requires_all = &["destination_port", "destination_protocol"])]
    pub destination_host: Option<String>,

    #[clap(long, env = "AB_DESTINATION_PORT")]
    pub destination_port: Option<u16>,

    #[clap(long, env = "AB_DESTINATION_PROTOCOL")]
    pub destination_protocol: Option<String>,

    #[clap(long, env = "AB_STAT_INTERVAL", default_value = "5")]
    pub stat_interval: u64,
}

impl Config {
    pub fn show_config(&self) {
        debug!("Log Level: {}", self.log_level);
        debug!("Source Host: {}", self.source_host);
        debug!("Source Port: {}", self.source_port);
        debug!("Source Protocol: {}", self.source_protocol);
        debug!("Destination Host: {:?}", self.destination_host);
        debug!("Destination Port: {:?}", self.destination_port);
        debug!("Destination Protocol: {:?}", self.destination_protocol);
        debug!("Stat Interval: {}", self.stat_interval);
        debug!("Would start output server: {}", self.is_destination_set());
    }

    #[must_use]
    pub fn get_log_level(&self) -> &str {
        &self.log_level
    }

    #[must_use]
    pub fn get_source_host(&self) -> &str {
        &self.source_host
    }

    #[must_use]
    pub const fn get_source_port(&self) -> u16 {
        self.source_port
    }

    #[must_use]
    pub fn get_source_protocol(&self) -> &str {
        &self.source_protocol
    }

    #[must_use]
    pub const fn get_destination_host(&self) -> &Option<String> {
        &self.destination_host
    }

    #[must_use]
    pub const fn get_destination_port(&self) -> Option<u16> {
        self.destination_port
    }

    #[must_use]
    pub const fn get_destination_protocol(&self) -> &Option<String> {
        &self.destination_protocol
    }

    #[must_use]
    pub const fn get_stat_interval(&self) -> u64 {
        self.stat_interval
    }

    #[must_use]
    pub const fn is_destination_set(&self) -> bool {
        self.destination_host.is_some()
            && self.destination_port.is_some()
            && self.destination_protocol.is_some()
    }
}
