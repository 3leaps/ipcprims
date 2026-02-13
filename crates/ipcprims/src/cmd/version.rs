use crate::cmd::VersionArgs;
use crate::exit::{CliResult, SUCCESS};

pub fn run(args: VersionArgs) -> CliResult<i32> {
    if !args.extended {
        println!("ipcprims {}", env!("CARGO_PKG_VERSION"));
        return Ok(SUCCESS);
    }

    println!("name: ipcprims");
    println!("version: {}", env!("CARGO_PKG_VERSION"));
    println!("target_os: {}", std::env::consts::OS);
    println!("target_arch: {}", std::env::consts::ARCH);
    println!(
        "rustc: {}",
        option_env!("RUSTC_VERSION").unwrap_or("unknown")
    );
    println!("git_hash: {}", option_env!("GIT_HASH").unwrap_or("unknown"));
    println!(
        "features: peer={}, schema={}, async={}, cli=true",
        cfg!(feature = "peer"),
        cfg!(feature = "schema"),
        cfg!(feature = "async")
    );
    println!("rsfulmen: not-linked (v0.1.0 scaffold)");

    Ok(SUCCESS)
}
