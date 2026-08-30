use clap::Parser;

#[derive(Parser)]
#[command(
    name = "screamwire-sender",
    about = "Scream audio sender for PipeWire",
    version = concat!(
        env!("CARGO_PKG_VERSION"),
        " (",
        env!("GIT_COMMIT_HASH"),
        ")"
    )
)]
pub struct Cli {
    /// Enable verbose output
    #[arg(long)]
    pub verbose: bool,

    /// Path to a TOML configuration file
    #[arg(long)]
    pub config: Option<String>,

    /// Multicast/unicast target address (default: 239.255.77.77:4010)
    #[arg(long)]
    pub target_addr: Option<String>,

    /// Address to bind the sender socket to (default: 0.0.0.0:0)
    #[arg(long)]
    pub sender_bind_addr: Option<String>,

    /// Audio sample rate in Hz (default: 48000)
    #[arg(long)]
    pub rate: Option<u32>,

    /// Audio bits (default: 16)
    #[arg(long)]
    pub bits: Option<u32>,

    /// Number of audio channels. Common setups should work as expected (default: 2)
    #[arg(long)]
    pub channels: Option<u32>,

    /// Enable Voice Activity Detection (default: true)
    #[arg(long)]
    pub vad_enable: Option<bool>,

    /// VAD mode: off, quick-1024, full-simd
    #[arg(long)]
    pub vad_mode: Option<String>,

    /// Silence duration in seconds before pausing transmission (default: 1.0)
    #[arg(long)]
    pub vad_silence: Option<f64>,

    /// Fade-in duration in ms (0 disables fade)
    #[arg(long)]
    pub fade_ms: Option<u32>,

    /// Name of an existing sink to capture from (instead of creating a virtual sink)
    #[arg(long)]
    pub sink: Option<String>,

    /// List all available sinks and exit
    #[arg(long)]
    pub list_sinks: bool,

    /// Generate a default configuration file at ~/.config/screamwire/config.toml and exit
    #[arg(long)]
    pub generate_config: bool,
}
