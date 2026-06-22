use clap::Parser;
use log::{debug, info};
use ringbuf::{HeapRb, traits::Split};
use std::thread;

mod cli;
mod config;
mod pw;
mod scream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    // Initialize logger
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    debug!("Verbose mode enabled");

    let mut cfg = config::Config::load(&cli)?;
    cfg.apply_cli_overrides(&cli);

    info!("ScreamWire sender starting...");
    info!("Multicast target: {}", cfg.target_addr);

    let buffer_size = scream::PACKET_SIZE * cfg.ring_buffer_packets;
    let rb = HeapRb::<u8>::new(buffer_size);
    let (producer, consumer) = rb.split();

    let target_addr: std::net::SocketAddr = cfg.target_addr.parse()?;
    let bind_addr: std::net::SocketAddr = cfg.sender_bind_addr.parse()?;
    let _sender_thread = thread::spawn(move || scream::send_loop(consumer, target_addr, bind_addr));

    pw::run_virtual_sink(producer, cfg.rate, cfg.channels)?;

    Ok(())
}
