use serde_json::Value;
use std::{sync::mpsc, time::Duration};
use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};

fn value_number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

pub(crate) fn capture_rect(window: &tauri::WebviewWindow, value: &Value) -> Result<String, String> {
    let rect = value
        .get("canvas")
        .or_else(|| value.get("wrapper"))
        .ok_or_else(|| format!("capture payload has no canvas/wrapper rect: {value}"))?;
    let x = value_number(rect, "x").ok_or_else(|| format!("missing rect.x in payload: {value}"))?;
    let y = value_number(rect, "y").ok_or_else(|| format!("missing rect.y in payload: {value}"))?;
    let width =
        value_number(rect, "w").ok_or_else(|| format!("missing rect.w in payload: {value}"))?;
    let height =
        value_number(rect, "h").ok_or_else(|| format!("missing rect.h in payload: {value}"))?;
    if width < 64.0 || height < 64.0 {
        return Err(format!(
            "DevTools clip is too small: x={x}, y={y}, w={width}, h={height}"
        ));
    }
    let params = serde_json::json!({
        "format": "png",
        "fromSurface": true,
        "captureBeyondViewport": false,
        "clip": {
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "scale": 1
        }
    })
    .to_string();
    let (outer_tx, outer_rx) = mpsc::channel();
    window
        .with_webview(move |platform_webview| {
            let result = (|| -> Result<String, String> {
                let controller = platform_webview.controller();
                let webview = unsafe {
                    controller
                        .CoreWebView2()
                        .map_err(|error| format!("CoreWebView2 unavailable: {error}"))?
                };
                let (tx, rx) = mpsc::channel();
                let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
                    move |error_code, result| {
                        let status = error_code
                            .as_ref()
                            .map(|_| ())
                            .map_err(|error| error.to_string());
                        let _ = tx.send((status, result));
                        error_code
                    },
                ));
                let method = CoTaskMemPWSTR::from("Page.captureScreenshot");
                let params = CoTaskMemPWSTR::from(params.as_str());
                unsafe {
                    webview
                        .CallDevToolsProtocolMethod(
                            *method.as_ref().as_pcwstr(),
                            *params.as_ref().as_pcwstr(),
                            &handler,
                        )
                        .map_err(|error| format!("CallDevToolsProtocolMethod failed: {error}"))?;
                }
                let (error_code, raw) = webview2_com::wait_with_pump(rx)
                    .map_err(|error| format!("DevTools callback failed: {error}"))?;
                if let Err(error) = error_code {
                    return Err(format!("DevTools returned error: {error}; raw={raw}"));
                }
                let response = serde_json::from_str::<Value>(&raw).map_err(|error| {
                    format!("DevTools returned invalid JSON: {error}; raw={raw}")
                })?;
                let data = response
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("DevTools response has no data field: {response}"))?;
                Ok(format!("data:image/png;base64,{data}"))
            })();
            let _ = outer_tx.send(result);
        })
        .map_err(|error| format!("with_webview failed: {error}"))?;
    outer_rx
        .recv_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("DevTools capture timed out: {error}"))?
}
