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

// Ring buffer size (number of packets, ~60 ms buffering)
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
