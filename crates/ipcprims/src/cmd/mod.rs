use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::exit::CliResult;
use crate::output::OutputFormat;

pub mod doctor;
pub mod echo;
pub mod envinfo;
pub mod info;
pub mod listen;
pub mod send;
pub mod version;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start an echo server.
    Echo(EchoArgs),
    /// Send a single frame.
    Send(SendArgs),
    /// Listen and print received frames.
    Listen(ListenArgs),
    /// Show version information.
    Version(VersionArgs),
    /// Probe a peer connection and print negotiated metadata.
    Info(InfoArgs),
    /// Run local environment health checks.
    Doctor(DoctorArgs),
    /// Print build and environment diagnostics.
    Envinfo(EnvinfoArgs),
}

pub fn run(command: Command, format: OutputFormat) -> CliResult<i32> {
    match command {
        Command::Echo(args) => echo::run(args, format),
        Command::Send(args) => send::run(args, format),
        Command::Listen(args) => listen::run(args, format),
        Command::Version(args) => version::run(args),
        Command::Info(args) => info::run(args, format),
        Command::Doctor(args) => doctor::run(args, format),
        Command::Envinfo(args) => envinfo::run(args, format),
    }
}

#[derive(Args, Debug)]
pub struct EchoArgs {
    /// Socket path to bind.
    pub path: PathBuf,
    /// Channels to echo (comma-separated). Default: all negotiated channels.
    #[arg(long, value_delimiter = ',')]
    pub channels: Option<Vec<u16>>,
    /// Schema directory for payload validation.
    #[arg(long, value_name = "DIR")]
    pub validate: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct SendArgs {
    /// Socket path to connect to.
    pub path: PathBuf,
    /// Channel to send on.
    #[arg(long, short = 'c', default_value = "1")]
    pub channel: u16,
    /// JSON payload.
    #[arg(long, conflicts_with_all = ["data", "file"])]
    pub json: Option<String>,
    /// Raw string payload.
    #[arg(long, conflicts_with_all = ["json", "file"])]
    pub data: Option<String>,
    /// Read payload from file.
    #[arg(long, conflicts_with_all = ["json", "data"])]
    pub file: Option<PathBuf>,
    /// Wait for one response frame and print it.
    #[arg(long)]
    pub wait: bool,
    /// Maximum time to wait for response when --wait is set (e.g. 5s, 500ms).
    #[arg(long, default_value = "5s")]
    pub wait_timeout: String,
}

#[derive(Args, Debug)]
pub struct ListenArgs {
    /// Socket path to bind.
    pub path: PathBuf,
    /// Filter to specific channels (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub channels: Option<Vec<u16>>,
    /// Exit after receiving N frames.
    #[arg(long)]
    pub count: Option<usize>,
}

#[derive(Args, Debug)]
pub struct VersionArgs {
    /// Show extended build provenance.
    #[arg(long)]
    pub extended: bool,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    /// Socket path to connect to.
    pub path: PathBuf,
    /// Connection timeout (e.g. 5s, 500ms).
    #[arg(long, default_value = "5s")]
    pub timeout: String,
}

#[derive(Args, Debug, Default)]
pub struct DoctorArgs {}

#[derive(Args, Debug, Default)]
pub struct EnvinfoArgs {}
