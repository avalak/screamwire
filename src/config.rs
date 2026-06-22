use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    // Network
    #[serde(default = "default_target_addr")]
    pub target_addr: String,

    #[serde(default = "default_sender_bind_addr")]
    pub sender_bind_addr: String,

    // Audio
    #[serde(default = "default_rate")]
    pub rate: u32,

    #[serde(default = "default_channels")]
    pub channels: u32,

    // VAD (Voice Activity Detection)
    #[serde(default = "default_vad_threshold")]
    pub vad_threshold: u16,

    #[serde(default = "default_silence_packets")]
    pub silence_packets: u32,

    // Ring buffer
    #[serde(default = "default_ring_buffer_packets")]
    pub ring_buffer_packets: usize,
}

fn default_target_addr() -> String {
    "239.255.77.77:4010".to_string()
}

fn default_sender_bind_addr() -> String {
    "0.0.0.0:0".to_string()
}

fn default_rate() -> u32 {
    crate::scream::RATE
}

fn default_channels() -> u32 {
    crate::scream::CHANNELS
}

fn default_vad_threshold() -> u16 {
    // 0 = VAD disabled, continuous transmission.
    // 1 = wake on any non‑zero sample (complete digital silence is 0).
    // Increase to ignore low-level background noise (e.g., 50-200).
    1
}

fn default_silence_packets() -> u32 {
    // 1 second of silence = ceil(RATE / (AUDIO_PAYLOAD_SIZE / FRAME_BYTES))
    // 48000 / (1152 / 4) = 48000 / 288 = 166.67 -> 167 packets
    167
}

fn default_ring_buffer_packets() -> usize {
    10
}

impl Config {
    /// Load configuration from a file, or return defaults if no path is given.
    pub fn load(cli: &super::cli::Cli) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(ref path) = cli.config {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read config '{}': {}", path, e))?;
            let cfg: Config = toml::from_str(&content)
                .map_err(|e| format!("invalid config '{}': {}", path, e))?;
            Ok(cfg)
        } else {
            Ok(Config {
                target_addr: default_target_addr(),
                sender_bind_addr: default_sender_bind_addr(),
                rate: default_rate(),
                channels: default_channels(),
                vad_threshold: default_vad_threshold(),
                silence_packets: default_silence_packets(),
                ring_buffer_packets: default_ring_buffer_packets(),
            })
        }
    }

    /// Override configuration fields with explicit CLI arguments.
    pub fn apply_cli_overrides(&mut self, cli: &super::cli::Cli) {
        if let Some(ref bind_addr) = cli.sender_bind_addr {
            self.sender_bind_addr = bind_addr.clone();
        }
    }
}
