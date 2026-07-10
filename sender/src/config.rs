use screamwire_common::scream::default_target_addr;
use screamwire_common::types::{DEFAULT_BITS, DEFAULT_CHANNELS, DEFAULT_RATE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    // Network
    #[serde(default = "default_target_addr")]
    pub target_addr: String,

    #[serde(default = "default_sender_bind_addr")]
    pub sender_bind_addr: String,

    // Audio
    #[serde(default = "default_rate")]
    pub rate: u32,

    #[serde(default = "default_bits")]
    pub bits: u32,

    #[serde(default = "default_channels")]
    pub channels: u32,

    // VAD (Voice Activity Detection)
    #[serde(default = "default_vad_enable")]
    pub vad_enable: bool,

    #[serde(default = "default_vad_threshold")]
    pub vad_threshold: u16,

    /// Silence duration in seconds before pausing transmission.
    #[serde(default = "default_vad_silence")]
    pub vad_silence: f64,

    // Existing sink capture (optional)
    #[serde(default)]
    pub sink_name: Option<String>,
}

fn default_sender_bind_addr() -> String {
    "0.0.0.0:0".to_string()
}

fn default_rate() -> u32 {
    DEFAULT_RATE
}

fn default_bits() -> u32 {
    DEFAULT_BITS
}

fn default_channels() -> u32 {
    DEFAULT_CHANNELS
}

fn default_vad_enable() -> bool {
    true
}

fn default_vad_threshold() -> u16 {
    1
}

fn default_vad_silence() -> f64 {
    1.0
}

/// Return the default configuration file path (`$XDG_CONFIG_HOME/screamwire/config.toml`
/// or `~/.config/screamwire/config.toml`) if the file exists, otherwise `None`.
pub fn default_config_path() -> Option<PathBuf> {
    let base = if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        return None; // cannot determine config directory
    };
    let path = base.join("screamwire").join("config.toml");
    if path.exists() { Some(path) } else { None }
}

impl Config {
    /// Load configuration from a file, falling back to the default XDG path or hardcoded defaults.
    pub fn load(cli: &super::cli::Cli) -> Result<Self, Box<dyn std::error::Error>> {
        let explicit_path = cli.config.as_deref();
        let config_path = explicit_path
            .map(PathBuf::from)
            .or_else(default_config_path);

        if let Some(path) = config_path {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read config '{}': {}", path.display(), e))?;
            let cfg: Config = toml::from_str(&content)
                .map_err(|e| format!("invalid config '{}': {}", path.display(), e))?;
            Ok(cfg)
        } else {
            Ok(Config {
                target_addr: default_target_addr(),
                sender_bind_addr: default_sender_bind_addr(),
                rate: default_rate(),
                bits: default_bits(),
                channels: default_channels(),
                vad_enable: default_vad_enable(),
                vad_threshold: default_vad_threshold(),
                vad_silence: default_vad_silence(),
                sink_name: None,
            })
        }
    }

    /// Override configuration fields with explicit CLI arguments.
    pub fn apply_cli_overrides(&mut self, cli: &super::cli::Cli) {
        if let Some(ref addr) = cli.target_addr {
            self.target_addr = addr.clone();
        }
        if let Some(ref bind) = cli.sender_bind_addr {
            self.sender_bind_addr = bind.clone();
        }
        if let Some(rate) = cli.rate {
            self.rate = rate;
        }
        if let Some(bits) = cli.bits {
            self.bits = bits;
        }
        if let Some(ch) = cli.channels {
            self.channels = ch;
        }
        if let Some(enable) = cli.vad_enable {
            self.vad_enable = enable;
        }
        if let Some(ref sink) = cli.sink {
            self.sink_name = Some(sink.clone());
        }
    }

    /// Write a default configuration file to the given path.
    pub fn generate_default(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if path.exists() {
            return Err(format!("File already exists: {}", path.display()).into());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory '{}': {}", parent.display(), e))?;
        }
        let default_config = Self::default_config();
        let toml_string = toml::to_string_pretty(&default_config)
            .map_err(|e| format!("Failed to serialize default config: {}", e))?;
        std::fs::write(path, toml_string)
            .map_err(|e| format!("Failed to write config to '{}': {}", path.display(), e))?;
        println!("Default configuration written to {}", path.display());
        Ok(())
    }

    fn default_config() -> Self {
        Config {
            target_addr: default_target_addr(),
            sender_bind_addr: default_sender_bind_addr(),
            rate: default_rate(),
            bits: default_bits(),
            channels: default_channels(),
            vad_enable: default_vad_enable(),
            vad_threshold: default_vad_threshold(),
            vad_silence: default_vad_silence(),
            sink_name: None,
        }
    }
}
