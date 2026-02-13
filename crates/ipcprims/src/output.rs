use std::io::{IsTerminal, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ValueEnum;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use ipcprims_frame::{Frame, COMMAND, CONTROL, DATA, ERROR, TELEMETRY};
use serde::Serialize;

#[derive(Clone, Debug, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    Table,
    Pretty,
    Raw,
}

impl OutputFormat {
    pub fn default_for_stdout() -> Self {
        if std::io::stdout().is_terminal() {
            Self::Table
        } else {
            Self::Json
        }
    }
}

#[derive(Serialize)]
struct FrameOutput<'a> {
    schema_id: &'a str,
    channel: u16,
    channel_name: &'a str,
    payload_size: usize,
    payload: String,
    peer_id: &'a str,
    timestamp: String,
}

pub fn print_frame(frame: &Frame, peer_id: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let out = FrameOutput {
                schema_id: "https://schemas.3leaps.dev/ipcprims/cli/v1/frame-received.schema.json",
                channel: frame.channel,
                channel_name: channel_name(frame.channel),
                payload_size: frame.payload.len(),
                payload: payload_preview(frame.payload.as_ref()),
                peer_id,
                timestamp: now_unix_seconds(),
            };
            println!(
                "{}",
                serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string())
            );
        }
        OutputFormat::Table => {
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec!["CHANNEL", "SIZE", "PEER", "PAYLOAD"])
                .add_row(vec![
                    channel_name(frame.channel).to_string(),
                    frame.payload.len().to_string(),
                    peer_id.to_string(),
                    payload_preview(frame.payload.as_ref()),
                ]);
            println!("{table}");
        }
        OutputFormat::Pretty => {
            println!(
                "channel={} ({}) size={} peer={} payload={}",
                frame.channel,
                channel_name(frame.channel),
                frame.payload.len(),
                peer_id,
                payload_preview(frame.payload.as_ref())
            );
        }
        OutputFormat::Raw => {
            print_raw(frame.payload.as_ref());
        }
    }
}

pub fn print_raw(data: &[u8]) {
    let mut out = std::io::stdout();
    let _ = out.write_all(data);
    let _ = out.flush();
}

pub fn channel_name(channel: u16) -> &'static str {
    match channel {
        CONTROL => "CONTROL",
        COMMAND => "COMMAND",
        DATA => "DATA",
        TELEMETRY => "TELEMETRY",
        ERROR => "ERROR",
        _ => "USER",
    }
}

fn payload_preview(payload: &[u8]) -> String {
    match std::str::from_utf8(payload) {
        Ok(text) => text.to_string(),
        Err(_) => format!("<binary {} bytes>", payload.len()),
    }
}

fn now_unix_seconds() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
