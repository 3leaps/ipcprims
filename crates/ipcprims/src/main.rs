mod cmd;
mod exit;
mod logging;
mod output;

use clap::Parser;

use crate::cmd::Command;
use crate::logging::{init_logging, LogFormat, LogLevel};
use crate::output::OutputFormat;

#[derive(Parser, Debug)]
#[command(name = "ipcprims", version, about = "IPC primitives CLI")]
struct Cli {
    /// Output format.
    #[arg(long, value_name = "FORMAT", global = true)]
    format: Option<OutputFormat>,

    /// Log output format (stderr).
    #[arg(long, value_name = "FORMAT", default_value = "text", global = true)]
    log_format: LogFormat,

    /// Minimum log level (stderr).
    #[arg(long, value_name = "LEVEL", default_value = "info", global = true)]
    log_level: LogLevel,

    #[command(subcommand)]
    command: Command,
}

fn main() {
    let cli = Cli::parse();
    init_logging(cli.log_format, cli.log_level);

    let format = cli.format.unwrap_or_else(OutputFormat::default_for_stdout);
    let result = cmd::run(cli.command, format);

    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(err.code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_send_subcommand() {
        let cli = Cli::try_parse_from([
            "ipcprims",
            "send",
            "/tmp/test.sock",
            "--channel",
            "1",
            "--data",
            "hello",
        ])
        .expect("send args should parse");

        assert!(matches!(cli.command, Command::Send(_)));
    }

    #[test]
    fn rejects_conflicting_payload_args() {
        let err = Cli::try_parse_from([
            "ipcprims",
            "send",
            "/tmp/test.sock",
            "--json",
            "{\"x\":1}",
            "--data",
            "hello",
        ])
        .expect_err("conflicting args should fail");

        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn parses_info_subcommand() {
        let cli = Cli::try_parse_from(["ipcprims", "info", "/tmp/test.sock", "--timeout", "3s"])
            .expect("info args should parse");
        assert!(matches!(cli.command, Command::Info(_)));
    }
}
