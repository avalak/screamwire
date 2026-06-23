use clap::Parser;
use log::{debug, info};
use ringbuf::{HeapRb, traits::Split};
use std::thread;

mod cli;
mod config;
mod pw;
mod scream;
mod vad;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    // Initialize logger
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    debug!("Verbose mode enabled");

    let mut cfg = config::Config::load(&cli)?;
    cfg.apply_cli_overrides(&cli);

    // Retrieve the list of available sinks
    let available_sinks = pw::get_sink_names();

    if cli.list_sinks {
        if available_sinks.is_empty() {
            println!("No audio sinks found.");
        } else {
            println!("Available audio sinks:");
            for name in available_sinks {
                println!("  - {}", name);
            }
        }
        return Ok(());
    }

    if cli.generate_config {
        let config_path = config::default_config_path().unwrap_or_else(|| {
            let home = std::env::var("HOME").expect("HOME not set");
            std::path::PathBuf::from(home).join(".config/screamwire/config.toml")
        });
        config::Config::generate_default(&config_path)?;
        return Ok(());
    }

    info!("ScreamWire sender starting...");

    // Create the ring buffer and start the network sender thread
    let buffer_size = scream::PACKET_SIZE * cfg.ring_buffer_packets;
    let rb = HeapRb::<u8>::new(buffer_size);
    let (producer, consumer) = rb.split();

    let target_addr: std::net::SocketAddr = cfg.target_addr.parse()?;
    let bind_addr: std::net::SocketAddr = cfg.sender_bind_addr.parse()?;

    let frame_bytes = (cfg.bits as usize / 8) * cfg.channels as usize;
    let vad_config = vad::VadConfig {
        threshold: cfg.vad_threshold,
        silence_packets: cfg.silence_packets,
        active_sleep_ms: cfg.active_sleep_ms,
        idle_sleep_ms: cfg.idle_sleep_ms,
        frame_bytes,
    };
    // Start sender thread

    let _sender_thread = thread::spawn(move || {
        scream::send_loop(
            consumer,
            target_addr,
            bind_addr,
            cfg.rate,
            scream::BITS,
            cfg.channels,
            vad_config,
        )
    });

    // Determine the mode and launch the audio stream
    let sink_name = cfg.sink_name.clone();
    if let Some(ref name) = sink_name
        && !name.is_empty()
    {
        if !available_sinks.contains(name) {
            eprintln!(
                "Error: sink '{}' not found. Use --list-sinks to see available names.",
                name
            );
            std::process::exit(1);
        }
        info!("Using existing sink: {}", name);
        pw::run_audio_stream(producer, cfg.rate, cfg.channels, Some(name.clone()))?;
    } else {
        pw::run_audio_stream(producer, cfg.rate, cfg.channels, None)?;
    }

    Ok(())
}
