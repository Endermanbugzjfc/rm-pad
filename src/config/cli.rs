use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::orientation::Orientation;

#[derive(Parser)]
#[command(name = "rm-pad")]
#[command(about = "Forward reMarkable tablet input to your computer")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    // All options are `global = true` so they are accepted both before and
    // after a subcommand (e.g. `rm-pad dump pen --host X --orientation Y`),
    // not only before it.

    /// reMarkable host (IP or hostname)
    #[arg(long, env = "RMPAD_HOST", global = true)]
    pub host: Option<String>,

    /// SSH key path for authentication
    #[arg(long, global = true)]
    pub key_path: Option<String>,

    /// SSH password (if set, key_path is ignored)
    #[arg(long, env = "RMPAD_PASSWORD", global = true)]
    pub password: Option<String>,

    /// Pen input device path on reMarkable
    #[arg(long, global = true)]
    pub pen_device: Option<String>,

    /// Touch input device path on reMarkable
    #[arg(long, global = true)]
    pub touch_device: Option<String>,

    /// Run touch input only (no pen)
    #[arg(long, global = true)]
    pub touch_only: bool,

    /// Run pen input only (no touch)
    #[arg(long, global = true)]
    pub pen_only: bool,

    /// Grab input exclusively [default: true]
    #[arg(long, global = true)]
    pub grab_input: bool,

    /// Don't grab input (tablet UI will also see input)
    #[arg(long, global = true)]
    pub no_grab_input: bool,

    /// Disable palm rejection
    #[arg(long, global = true)]
    pub no_palm_rejection: bool,

    /// Palm rejection grace period in milliseconds
    #[arg(long, global = true)]
    pub palm_grace_ms: Option<u64>,

    /// Screen orientation (portrait, landscape-right, landscape-left, inverted)
    #[arg(long, value_parser = clap::value_parser!(Orientation), global = true)]
    pub orientation: Option<Orientation>,

    /// Path to config file
    #[arg(long, env = "RMPAD_CONFIG", global = true)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Dump raw input events for debugging
    Dump {
        /// Device to dump: "touch" or "pen"
        device: String,
    },
}
