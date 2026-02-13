use std::path::PathBuf;

use serde::Serialize;

use crate::cmd::DoctorArgs;
use crate::exit::{CliResult, HEALTH_CHECK_FAILED, SUCCESS};
use crate::output::OutputFormat;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Fail,
    Warn,
    Info,
    Skip,
}

#[derive(Debug, Serialize)]
struct CheckResult {
    name: String,
    status: CheckStatus,
    detail: String,
}

#[derive(Debug, Serialize)]
struct DoctorOutput {
    schema_id: &'static str,
    checks: Vec<CheckResult>,
    overall: &'static str,
}

pub fn run(_args: DoctorArgs, format: OutputFormat) -> CliResult<i32> {
    let mut checks = vec![
        platform_transport_check(),
        temp_dir_writable_check(),
        rsfulmen_alignment_check(),
        compiled_features_check(),
    ];

    checks.push(schema_dir_check());

    let has_fail = checks.iter().any(|c| matches!(c.status, CheckStatus::Fail));
    let overall = if has_fail { "fail" } else { "pass" };

    let output = DoctorOutput {
        schema_id: "https://schemas.3leaps.dev/ipcprims/cli/v1/doctor-report.schema.json",
        checks,
        overall,
    };

    print_doctor(&output, format);

    if has_fail {
        Ok(HEALTH_CHECK_FAILED)
    } else {
        Ok(SUCCESS)
    }
}

fn rsfulmen_alignment_check() -> CheckResult {
    CheckResult {
        name: "rsfulmen_alignment".to_string(),
        status: CheckStatus::Warn,
        detail: "using local rsfulmen-aligned exit codes in v0.1.0 scaffold".to_string(),
    }
}

fn print_doctor(output: &DoctorOutput, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string())
            );
        }
        OutputFormat::Table | OutputFormat::Pretty => {
            println!("ipcprims doctor\n");
            for c in &output.checks {
                println!(
                    "  [{:>4}] {:<22} {}",
                    status_text(c.status),
                    c.name,
                    c.detail
                );
            }
            if output.overall == "pass" {
                println!("\n  Result: all checks passed");
            } else {
                println!("\n  Result: one or more checks failed");
            }
        }
        OutputFormat::Raw => {
            println!("{}", output.overall);
        }
    }
}

fn status_text(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "PASS",
        CheckStatus::Fail => "FAIL",
        CheckStatus::Warn => "WARN",
        CheckStatus::Info => "INFO",
        CheckStatus::Skip => "SKIP",
    }
}

fn platform_transport_check() -> CheckResult {
    #[cfg(unix)]
    {
        CheckResult {
            name: "platform_transport".to_string(),
            status: CheckStatus::Pass,
            detail: "Unix domain sockets available".to_string(),
        }
    }

    #[cfg(not(unix))]
    {
        // v0.1.0 transport currently targets Unix domain sockets only.
        // This explicit capability probe keeps doctor fail-closed on unsupported
        // platforms instead of emitting an ambiguous warning.
        CheckResult {
            name: "platform_transport".to_string(),
            status: CheckStatus::Fail,
            detail: "native non-Unix transport backend unavailable (named pipes not implemented)"
                .to_string(),
        }
    }
}

fn temp_dir_writable_check() -> CheckResult {
    #[cfg(unix)]
    {
        use ipcprims_transport::UnixDomainSocket;
        let dir = PathBuf::from(format!(
            "/tmp/ipcprims-doctor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("doctor.sock");
        let result = UnixDomainSocket::bind(&sock);
        let _ = std::fs::remove_dir_all(&dir);

        match result {
            Ok(_) => CheckResult {
                name: "temp_dir_writable".to_string(),
                status: CheckStatus::Pass,
                detail: "/tmp socket bind succeeded".to_string(),
            },
            Err(err) => CheckResult {
                name: "temp_dir_writable".to_string(),
                status: CheckStatus::Fail,
                detail: format!("/tmp socket bind failed: {err}"),
            },
        }
    }

    #[cfg(not(unix))]
    {
        CheckResult {
            name: "temp_dir_writable".to_string(),
            status: CheckStatus::Skip,
            detail: "temp socket check not implemented on this platform".to_string(),
        }
    }
}

fn compiled_features_check() -> CheckResult {
    let mut features = Vec::new();
    if cfg!(feature = "peer") {
        features.push("peer");
    }
    if cfg!(feature = "schema") {
        features.push("schema");
    }
    if cfg!(feature = "async") {
        features.push("async");
    }
    if cfg!(feature = "cli") {
        features.push("cli");
    }

    CheckResult {
        name: "compiled_features".to_string(),
        status: CheckStatus::Info,
        detail: features.join(", "),
    }
}

fn schema_dir_check() -> CheckResult {
    let path = match std::env::var("IPCPRIMS_SCHEMA_DIR") {
        Ok(value) => PathBuf::from(value),
        Err(_) => {
            return CheckResult {
                name: "schema_dir".to_string(),
                status: CheckStatus::Skip,
                detail: "IPCPRIMS_SCHEMA_DIR not set".to_string(),
            }
        }
    };

    if !path.exists() {
        return CheckResult {
            name: "schema_dir".to_string(),
            status: CheckStatus::Fail,
            detail: format!("{} does not exist", path.display()),
        };
    }

    if !path.is_dir() {
        return CheckResult {
            name: "schema_dir".to_string(),
            status: CheckStatus::Fail,
            detail: format!("{} is not a directory", path.display()),
        };
    }

    #[cfg(feature = "schema")]
    {
        match ipcprims_schema::SchemaRegistry::from_directory(&path) {
            Ok(_) => CheckResult {
                name: "schema_dir".to_string(),
                status: CheckStatus::Pass,
                detail: format!("{} loaded successfully", path.display()),
            },
            Err(err) => CheckResult {
                name: "schema_dir".to_string(),
                status: CheckStatus::Fail,
                detail: format!("{} failed schema load: {err}", path.display()),
            },
        }
    }

    #[cfg(not(feature = "schema"))]
    {
        CheckResult {
            name: "schema_dir".to_string(),
            status: CheckStatus::Skip,
            detail: "schema support not compiled in".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_output_has_overall_status() {
        let checks = vec![CheckResult {
            name: "x".to_string(),
            status: CheckStatus::Pass,
            detail: "ok".to_string(),
        }];
        let output = DoctorOutput {
            schema_id: "x",
            checks,
            overall: "pass",
        };
        let json = serde_json::to_string(&output).expect("doctor output should serialize");
        assert!(json.contains("\"overall\":\"pass\""));
    }
}
