use crate::FloatingBarPayload;
#[cfg(target_os = "macos")]
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub(crate) fn start(app: &tauri::AppHandle, payload: &FloatingBarPayload) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = app;
        super::windows::floating::start_external_floating_bar(payload)
    }
    #[cfg(target_os = "macos")]
    {
        let width = if payload.width.is_finite() {
            payload.width.clamp(132.0, 420.0)
        } else {
            180.0
        };
        if let Some(window) = app.get_webview_window(crate::FLOATING_BAR_ID) {
            window
                .set_size(tauri::LogicalSize::new(width, 38.0))
                .map_err(|e| e.to_string())?;
            window
                .emit("floating-bar-content", payload)
                .map_err(|e| e.to_string())?;
            return window.show().map_err(|e| e.to_string());
        }
        let window = WebviewWindowBuilder::new(
            app,
            crate::FLOATING_BAR_ID,
            WebviewUrl::App("floating.html".into()),
        )
        .title("Codex Switch")
        .inner_size(width, 38.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .accept_first_mouse(true)
        .position(24.0, 40.0)
        .build()
        .map_err(|e| e.to_string())?;
        window
            .emit("floating-bar-content", payload)
            .map_err(|e| e.to_string())
    }
}

pub(crate) fn stop(app: &tauri::AppHandle) {
    #[cfg(windows)]
    {
        super::windows::floating::stop_external_floating_bar();
    }
    use tauri::Manager;
    if let Some(window) = app.get_webview_window(crate::FLOATING_BAR_ID) {
        let _ = window.hide();
    }
}
