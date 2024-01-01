pub extern crate clap as clap;

extern crate sdre_rust_logging;

use clap::Parser;

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "ACARS Bridge", author, version, about, long_about = None)]
pub struct Config {
    #[clap(long, env = "AB_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    #[clap(long, env = "AB_SOURCE", default_value = "localhost")]
    pub source_host: String,

    #[clap(long, env = "AB_SOURCE_PORT")]
    pub source_port: u16,

    #[clap(long, env = "AB_SOURCE_PROTOCOL")]
    pub source_protocol: String,

    #[clap(long, env = "AB_DESTINATION", default_value = "acars_router")]
    pub destination_host: String,

    #[clap(long, env = "AB_DESTINATION_PORT")]
    pub destination_port: u16,

    #[clap(long, env = "AB_DESTINATION_PROTOCOL", default_value = "zmq")]
    pub destination_protocol: String,

    #[clap(long, env = "AB_STAT_INTERVAL", default_value = "5")]
    pub stat_interval: u64,
}

impl Config {
    pub fn show_config(&self) {
        info!("Log Level: {}", self.log_level);
        info!("Source Host: {}", self.source_host);
        info!("Source Port: {}", self.source_port);
        info!("Source Protocol: {}", self.source_protocol);
        info!("Destination Host: {}", self.destination_host);
        info!("Destination Port: {}", self.destination_port);
        info!("Destination Protocol: {}", self.destination_protocol);
        info!("Stat Interval: {}", self.stat_interval);
    }

    pub fn get_log_level(&self) -> &str {
        &self.log_level
    }

    pub fn get_source_host(&self) -> &str {
        &self.source_host
    }

    pub fn get_source_port(&self) -> u16 {
        self.source_port
    }

    pub fn get_source_protocol(&self) -> &str {
        &self.source_protocol
    }

    pub fn get_destination_host(&self) -> &str {
        &self.destination_host
    }

    pub fn get_destination_port(&self) -> u16 {
        self.destination_port
    }

    pub fn get_destination_protocol(&self) -> &str {
        &self.destination_protocol
    }

    pub fn get_stat_interval(&self) -> u64 {
        self.stat_interval
    }
}
