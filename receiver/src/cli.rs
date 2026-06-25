use clap::Parser;

/// Command-line interface for the ScreamWire receiver.
#[derive(Parser)]
#[command(
    name = "screamwire-receiver",
    about = "Scream audio receiver for PipeWire"
)]
pub struct Cli {
    /// Enable verbose (debug) logging.
    #[arg(long)]
    pub verbose: bool,

    /// Buffer size
    #[arg(long, default_value_t = 10)]
    pub buffer_size: u32,
}
