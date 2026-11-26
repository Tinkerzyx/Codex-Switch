use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlatformInfo {
    os: &'static str,
    arch: &'static str,
    version: &'static str,
    data_schema_version: u32,
    native_qr_capture: bool,
    file_credentials_supported: bool,
    credential_message: Option<String>,
}

#[tauri::command]
pub(crate) fn get_platform_info() -> PlatformInfo {
    let store = crate::core::auth_files::check_store(&crate::platform::codex_dir());
    PlatformInfo {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        version: env!("CARGO_PKG_VERSION"),
        data_schema_version: crate::storage::SCHEMA_VERSION,
        native_qr_capture: cfg!(windows),
        file_credentials_supported: store.is_ok(),
        credential_message: store.err(),
    }
}
