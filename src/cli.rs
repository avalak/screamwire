use clap::Parser;

#[derive(Parser)]
#[command(name = "screamwire", about = "Scream audio sender for PipeWire")]
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

    /// Number of audio channels (default: 2)
    #[arg(long)]
    pub channels: Option<u32>,

    /// VAD amplitude threshold (0 = disabled, 1 = wake on any non‑zero sample)
    #[arg(long)]
    pub vad_threshold: Option<u16>,

    /// Consecutive silent packets before pausing transmission (0 = disabled)
    #[arg(long)]
    pub silence_packets: Option<u32>,

    /// Ring buffer size in packets (default: 10)
    #[arg(long)]
    pub ring_buffer_packets: Option<usize>,

    /// Sleep duration in ms while actively transmitting (default: 4)
    #[arg(long)]
    pub active_sleep_ms: Option<u64>,

    /// Sleep duration in ms when VAD has paused transmission (default: 30)
    #[arg(long)]
    pub idle_sleep_ms: Option<u64>,
}
