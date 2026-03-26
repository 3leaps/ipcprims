#![cfg(feature = "cli")]

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use ipcprims_frame::{COMMAND, ERROR};
use ipcprims_peer::connect;

#[cfg(unix)]
fn unique_ipc_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!(
        "/tmp/icpcli-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
    dir
}

#[cfg(windows)]
fn unique_ipc_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!(
        "{}/icpcli-{tag}-{}-{}",
        std::env::temp_dir().display(),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
    dir
}

#[cfg(unix)]
fn unique_ipc_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.sock"))
}

#[cfg(windows)]
fn unique_ipc_path(_dir: &Path, name: &str) -> PathBuf {
    PathBuf::from(format!(
        r"\\.\pipe\ipcprims-cli-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos()
    ))
}

fn wait_for_connect(
    path: &Path,
    channels: &[u16],
    timeout: Duration,
) -> io::Result<ipcprims_peer::Peer> {
    let start = Instant::now();
    loop {
        match connect(path, channels) {
            Ok(peer) => return Ok(peer),
            Err(err) => {
                if start.elapsed() >= timeout {
                    return Err(io::Error::other(format!("connect timeout: {err}")));
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

#[test]
fn echo_validate_returns_error_and_continues_session() {
    let dir = unique_ipc_dir("echo-validate");
    let sock_path = unique_ipc_path(&dir, "echo");
    let schema_dir = dir.join("schemas");
    std::fs::create_dir_all(&schema_dir).expect("schema dir should be creatable");

    std::fs::write(
        schema_dir.join("command.schema.json"),
        r#"{
            "type": "object",
            "properties": {
                "ok": { "type": "boolean" }
            },
            "required": ["ok"]
        }"#,
    )
    .expect("schema file should be writable");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ipcprims"))
        .arg("--log-level")
        .arg("error")
        .arg("echo")
        .arg(&sock_path)
        .arg("--validate")
        .arg(&schema_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("echo command should start");

    let mut peer = wait_for_connect(&sock_path, &[COMMAND, ERROR], Duration::from_secs(5))
        .expect("client should connect to echo server");

    peer.send(COMMAND, br#"{"nope":true}"#)
        .expect("invalid frame should send");
    let error_frame = peer.recv_on(ERROR).expect("ERROR frame should be returned");
    let error_json: serde_json::Value =
        serde_json::from_slice(error_frame.payload.as_ref()).expect("error payload should be json");
    assert!(error_json
        .get("error")
        .and_then(|v| v.as_str())
        .map(|s| s.contains("schema validation error"))
        .unwrap_or(false));

    peer.send(COMMAND, br#"{"ok":true}"#)
        .expect("valid frame should send");
    let echoed = peer
        .recv_on(COMMAND)
        .expect("echo should continue after invalid frame");
    assert_eq!(echoed.payload.as_ref(), br#"{"ok":true}"#);

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
