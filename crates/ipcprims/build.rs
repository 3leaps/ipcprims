fn main() {
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=IPCPRIMS_BUILD_TARGET={target}");
    }
    println!("cargo:rerun-if-env-changed=TARGET");
}
