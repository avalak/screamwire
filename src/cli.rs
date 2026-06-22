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

    /// Address to bind the sender socket to (default: 0.0.0.0:0)
    #[arg(long)]
    pub sender_bind_addr: Option<String>,
}
