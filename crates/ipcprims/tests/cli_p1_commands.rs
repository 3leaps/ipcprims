#![cfg(all(unix, feature = "cli"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use ipcprims_peer::connect;

fn unique_temp_dir(tag: &str) -> PathBuf {
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

fn wait_for_connect(path: &Path, channels: &[u16], timeout: Duration) {
    let start = Instant::now();
    loop {
        if connect(path, channels).is_ok() {
            return;
        }
        if start.elapsed() >= timeout {
            panic!("connect timeout");
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn info_against_echo_server_outputs_connection_data() {
    let dir = unique_temp_dir("info");
    let sock_path = dir.join("echo.sock");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ipcprims"))
        .arg("--log-level")
        .arg("error")
        .arg("echo")
        .arg(&sock_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("echo command should start");

    wait_for_connect(&sock_path, &[1], Duration::from_secs(3));

    let output = Command::new(env!("CARGO_BIN_EXE_ipcprims"))
        .arg("--log-level")
        .arg("error")
        .arg("--format")
        .arg("json")
        .arg("info")
        .arg(&sock_path)
        .output()
        .expect("info should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("connection-info.schema.json"));
    assert!(stdout.contains("\"connected\":true"));

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn info_timeout_returns_124() {
    let missing = PathBuf::from(format!(
        "/tmp/icpcli-missing-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos()
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_ipcprims"))
        .arg("info")
        .arg(&missing)
        .arg("--timeout")
        .arg("1s")
        .output()
        .expect("info should run");

    assert_eq!(output.status.code(), Some(124));
}

#[test]
fn doctor_passes_on_clean_env() {
    let output = Command::new(env!("CARGO_BIN_EXE_ipcprims"))
        .arg("--format")
        .arg("json")
        .arg("doctor")
        .output()
        .expect("doctor should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("doctor-report.schema.json"));
}

#[test]
fn envinfo_reports_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_ipcprims"))
        .arg("--format")
        .arg("json")
        .arg("envinfo")
        .output()
        .expect("envinfo should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("envinfo.schema.json"));
    let payload: serde_json::Value =
        serde_json::from_str(&stdout).expect("envinfo should emit json");
    assert_eq!(
        payload.get("version").and_then(|v| v.as_str()),
        Some(env!("CARGO_PKG_VERSION"))
    );
}
