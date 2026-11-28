use std::path::PathBuf;
use tauri::{Manager, Runtime, WebviewWindowBuilder};

pub(crate) trait IsolatedWebSession {
    fn isolated_session(self, directory: PathBuf) -> Self;
}

impl<'a, R: Runtime, M: Manager<R>> IsolatedWebSession for WebviewWindowBuilder<'a, R, M> {
    fn isolated_session(self, directory: PathBuf) -> Self {
        isolated(self, directory)
    }
}

/// A stable, per-profile WKWebView store on macOS 14+, and a WebView2 directory on Windows.
pub(crate) fn isolated<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    directory: PathBuf,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "macos")]
    {
        let key = directory.file_name().unwrap_or_default().to_string_lossy();
        builder.data_store_identifier(store_identifier(&key))
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder.data_directory(directory)
    }
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn store_identifier(key: &str) -> [u8; 16] {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("com.codexswitch.app.web-session:{key}").as_bytes());
    let mut identifier = [0; 16];
    identifier.copy_from_slice(&digest[..16]);
    identifier
}

#[cfg(test)]
mod tests {
    #[test]
    fn store_identifiers_keep_the_existing_mac_login_mapping() {
        // Pin the pre-maintenance algorithm: changing this would log every Mac profile out.
        assert_eq!(
            super::store_identifier("0123456789abcdef0123456789abcdef"),
            [211, 198, 2, 116, 68, 163, 112, 0, 236, 39, 98, 237, 181, 56, 229, 135]
        );
        assert_ne!(super::store_identifier("a"), super::store_identifier("b"));
    }
}
