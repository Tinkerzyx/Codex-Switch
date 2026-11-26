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
        use sha2::{Digest, Sha256};
        let key = directory.file_name().unwrap_or_default().to_string_lossy();
        let digest = Sha256::digest(format!("com.codexswitch.app.web-session:{key}").as_bytes());
        let mut identifier = [0; 16];
        identifier.copy_from_slice(&digest[..16]);
        builder.data_store_identifier(identifier)
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder.data_directory(directory)
    }
}
