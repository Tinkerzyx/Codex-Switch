//! Use WebKit APIs, never filesystem guesses inside ~/Library/WebKit.
use super::MaintenanceLease;
use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_foundation::{NSDate, NSError, NSSet, NSUUID};
use objc2_web_kit::{WKWebsiteDataStore, WKWebsiteDataTypeDiskCache, WKWebsiteDataTypeMemoryCache};
use std::{sync::mpsc, time::Duration};

pub(super) fn clean(
    app: &tauri::AppHandle,
    id: &str,
    remove_store: bool,
    maintenance: MaintenanceLease,
) -> Result<(), String> {
    let identifier = crate::platform::web_session::store_identifier(id);
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let Some(main_thread) = MainThreadMarker::new() else {
            let _ = tx.send(Err("WebKit maintenance requires the main thread.".to_string()));
            return;
        };
        let uuid = NSUUID::from_bytes(identifier);
        if remove_store {
            let completion = RcBlock::new(move |error: *mut NSError| {
                // Keep the reservation until WebKit has actually finished, even on timeout.
                let _hold = &maintenance;
                let result = if error.is_null() { Ok(()) } else {
                    Err("WebKit could not remove the unused store; its marker was retained for retry.".into())
                };
                let _ = tx.send(result);
            });
            // SAFETY: macOS 14+ is the app minimum. UUID is the existing nonzero
            // app-specific mapping; leases exclude all managed views using this store.
            unsafe {
                WKWebsiteDataStore::removeDataStoreForIdentifier_completionHandler(
                    &uuid, &completion, main_thread,
                );
            }
        } else {
            // SAFETY: All WebKit objects are created and used on the main thread.
            let store = unsafe { WKWebsiteDataStore::dataStoreForIdentifier(&uuid, main_thread) };
            // An explicit allowlist is essential: allWebsiteDataTypes would erase logins.
            let cache_types = unsafe {
                NSSet::from_slice(&[WKWebsiteDataTypeDiskCache, WKWebsiteDataTypeMemoryCache])
            };
            let held_store = store.clone();
            let completion = RcBlock::new(move || {
                let _hold = (&maintenance, &held_store);
                let _ = tx.send(Ok(()));
            });
            // SAFETY: Only disposable disk/memory caches are selected. Cookies,
            // local storage, IndexedDB, credentials and service-worker data are excluded.
            unsafe {
                store.removeDataOfTypes_modifiedSince_completionHandler(
                    &cache_types, &NSDate::distantPast(), &completion,
                );
            }
        }
    }).map_err(|error| error.to_string())?;
    // Called only from maintenance workers, never from the UI/event-loop thread.
    rx.recv_timeout(Duration::from_secs(30)).map_err(|_| {
        "WebKit maintenance timed out; the session remains reserved until completion.".to_string()
    })?
}
