fn main() {
    println!("cargo:rerun-if-env-changed=CSW_RELEASE_VERSION");
    if let Ok(version) = std::env::var("CSW_RELEASE_VERSION") {
        assert_eq!(
            version,
            env!("CARGO_PKG_VERSION"),
            "Build version must match Cargo package metadata"
        );
    }
    tauri_build::build()
}
