use clap::Parser;
use log::{debug, info};
use ringbuf::{HeapRb, traits::Split};
use std::thread;

mod cli;
mod config;
mod event_bridge;
mod pw;
mod rt_log;
mod scanners;
mod udp_sender;
mod vad;

use crate::event_bridge::StreamEventBridge;
use screamwire_common::types::AudioParams;

use crate::config::BASE_BUFFER_SIZE;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    // Initialize logger
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    debug!("Verbose mode enabled");

    let mut cfg = config::Config::load(&cli)?;
    cfg.apply_cli_overrides(&cli);

    pipewire::init();
    let mainloop = pipewire::main_loop::MainLoopRc::new(None).expect("Failed to create main loop");
    let context =
        pipewire::context::ContextRc::new(&mainloop, None).expect("Failed to create context");

    // Retrieve the list of available sinks
    let available_sinks = pw::get_sink_names(mainloop.clone(), context.clone());

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
    let buffer_size = BASE_BUFFER_SIZE * 10;
    let rb = HeapRb::<u8>::new(buffer_size);
    let (producer, consumer) = rb.split();

    let target_addr: std::net::SocketAddr = cfg.target_addr.parse()?;
    let bind_addr: std::net::SocketAddr = cfg.sender_bind_addr.parse()?;

    let event_bridge = StreamEventBridge::new();
    let format = AudioParams {
        rate: cfg.rate,
        bits: cfg.bits,
        channels: cfg.channels,
    };

    // Calculate max silence bytes from seconds + audio params
    let frame_bytes = (cfg.bits as usize / 8) * cfg.channels as usize;
    let bytes_per_second = cfg.rate as usize * frame_bytes;
    let max_silence_bytes = (cfg.vad_silence * bytes_per_second as f64) as usize;

    let vad_config = vad::VadConfig {
        enabled: cfg.vad_enable,
        mode: cfg.vad_mode,
        threshold: if cfg.vad_enable { cfg.vad_threshold } else { 0 },
        max_silence_bytes,
    };
    let net_bridge = event_bridge.clone();

    // Start sender thread
    let _sender_thread = thread::spawn(move || {
        udp_sender::send_loop(consumer, target_addr, bind_addr, format, net_bridge)
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
        pw::run_audio_stream(
            mainloop,
            context,
            producer,
            format,
            Some(name.clone()),
            vad_config,
            event_bridge,
        )?;
    } else {
        pw::run_audio_stream(
            mainloop,
            context,
            producer,
            format,
            None,
            vad_config,
            event_bridge,
        )?;
    }

    Ok(())
}
