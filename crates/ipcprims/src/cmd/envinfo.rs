use std::collections::BTreeMap;

use serde::Serialize;

use crate::cmd::EnvinfoArgs;
use crate::exit::{CliResult, SUCCESS};
use crate::output::OutputFormat;

#[derive(Serialize)]
struct PlatformInfo {
    os: String,
    arch: String,
}

#[derive(Serialize)]
struct EnvInfoOutput {
    schema_id: &'static str,
    version: String,
    target: String,
    rust_version: String,
    git_hash: String,
    platform: PlatformInfo,
    features: Vec<String>,
    dependencies: BTreeMap<String, String>,
    environment: BTreeMap<String, Option<String>>,
}

pub fn run(_args: EnvinfoArgs, format: OutputFormat) -> CliResult<i32> {
    let mut deps = BTreeMap::new();
    deps.insert("clap".to_string(), "4.5".to_string());
    deps.insert("jsonschema".to_string(), "0.41".to_string());
    deps.insert("rsfulmen".to_string(), "not-linked".to_string());

    let mut env = BTreeMap::new();
    env.insert(
        "IPCPRIMS_SCHEMA_DIR".to_string(),
        std::env::var("IPCPRIMS_SCHEMA_DIR").ok(),
    );
    env.insert(
        "IPCPRIMS_LOG_LEVEL".to_string(),
        std::env::var("IPCPRIMS_LOG_LEVEL").ok(),
    );
    env.insert("RUST_LOG".to_string(), std::env::var("RUST_LOG").ok());

    let output = EnvInfoOutput {
        schema_id: "https://schemas.3leaps.dev/ipcprims/cli/v1/envinfo.schema.json",
        version: env!("CARGO_PKG_VERSION").to_string(),
        target: target_triple(),
        rust_version: option_env!("RUSTC_VERSION")
            .unwrap_or("unknown")
            .to_string(),
        git_hash: option_env!("GIT_HASH").unwrap_or("unknown").to_string(),
        platform: PlatformInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        },
        features: active_features(),
        dependencies: deps,
        environment: env,
    };

    print_envinfo(&output, format);
    Ok(SUCCESS)
}

fn target_triple() -> String {
    if let Some(target) = option_env!("IPCPRIMS_BUILD_TARGET") {
        return target.to_string();
    }

    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => "aarch64-apple-darwin".to_string(),
        ("x86_64", "macos") => "x86_64-apple-darwin".to_string(),
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu".to_string(),
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu".to_string(),
        ("x86_64", "windows") => "x86_64-pc-windows-msvc".to_string(),
        (arch, os) => format!("{arch}-unknown-{os}"),
    }
}

fn print_envinfo(output: &EnvInfoOutput, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string())
        ),
        OutputFormat::Table | OutputFormat::Pretty => {
            println!("ipcprims environment\n");
            println!("  Version:    {}", output.version);
            println!("  Target:     {}", output.target);
            println!("  Rust:       {}", output.rust_version);
            println!("  Git hash:   {}", output.git_hash);
            println!(
                "  Platform:   {} ({})",
                output.platform.os, output.platform.arch
            );
            println!("  Features:   {}", output.features.join(", "));
            println!("\n  Dependencies:");
            for (k, v) in &output.dependencies {
                println!("    {:<12} {}", k, v);
            }
            println!("\n  Environment:");
            for (k, v) in &output.environment {
                println!("    {:<20} {}", k, v.as_deref().unwrap_or("(not set)"));
            }
        }
        OutputFormat::Raw => println!("{}", output.version),
    }
}

fn active_features() -> Vec<String> {
    let mut features = Vec::new();
    if cfg!(feature = "peer") {
        features.push("peer".to_string());
    }
    if cfg!(feature = "schema") {
        features.push("schema".to_string());
    }
    if cfg!(feature = "async") {
        features.push("async".to_string());
    }
    if cfg!(feature = "cli") {
        features.push("cli".to_string());
    }
    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envinfo_json_has_schema_id() {
        let out = EnvInfoOutput {
            schema_id: "x",
            version: "0.1.0".to_string(),
            target: "a-b".to_string(),
            rust_version: "1.81.0".to_string(),
            git_hash: "abc".to_string(),
            platform: PlatformInfo {
                os: "macos".to_string(),
                arch: "aarch64".to_string(),
            },
            features: vec!["cli".to_string()],
            dependencies: BTreeMap::new(),
            environment: BTreeMap::new(),
        };

        let json = serde_json::to_string(&out).expect("envinfo output should serialize");
        assert!(json.contains("\"schema_id\""));
    }

    #[test]
    fn target_looks_like_triple() {
        let target = target_triple();
        assert!(target.split('-').count() >= 3);
    }
}
