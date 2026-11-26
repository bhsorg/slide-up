//! Native dialogs are OS-owned windows, not Tauri webviews. While any of our
//! windows is `alwaysOnTop` (picture-in-picture or a detached widget), Windows
//! keeps that window above the dialog, so the Open / Export / Import dialogs
//! appear hidden behind the player.
//!
//! [`TopmostGuard`] temporarily clears topmost on every app window for the
//! lifetime of a dialog and restores the exact previous z-order when dropped.
//! Frontend dialogs use the `begin_native_dialog` / `end_native_dialog`
//! commands, which keep a reference-counted guard alive across IPC calls.

use std::sync::atomic::{AtomicUsize, Ordering};

use tauri::{AppHandle, Manager};

/// Widgets first and `main` last so the player is restored above its own
/// widgets (matching the z-order logic in `widget.rs`).
const APP_WINDOWS: [&str; 4] = [
    "player-widget",
    "video-widget",
    "playlist-widget",
    "main",
];

#[must_use = "the guard must stay alive until the dialog closes"]
pub struct TopmostGuard {
    app: AppHandle,
    previous: Vec<(String, bool)>,
}

impl TopmostGuard {
    /// Clear always-on-top on all app windows until the guard is dropped.
    /// Call immediately before showing a native dialog.
    pub fn suspend(app: &AppHandle) -> Self {
        let mut previous = Vec::with_capacity(APP_WINDOWS.len());
        for label in APP_WINDOWS {
            let Some(window) = app.get_webview_window(label) else {
                continue;
            };
            let topmost = window.is_always_on_top().unwrap_or(false);
            if topmost {
                // Tauri/wry skip the native call when the style is unchanged,
                // so explicitly clear it to give the dialog a normal z-order.
                let _ = window.set_always_on_top(false);
            }
            previous.push((label.to_string(), topmost));
        }
        // Window messages are queued on the main thread. A blocking getter
        // flushes them so the z-order change is applied before the dialog is
        // created (tao uses SWP_ASYNCWINDOWPOS for SetWindowPos).
        flush_window_messages(app);
        Self {
            app: app.clone(),
            previous,
        }
    }
}

/// Block until queued window messages have been processed on the main thread.
fn flush_window_messages(app: &AppHandle) {
    for label in APP_WINDOWS.iter().rev() {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.is_always_on_top();
            return;
        }
    }
}

impl Drop for TopmostGuard {
    fn drop(&mut self) {
        for (label, topmost) in &self.previous {
            if !*topmost {
                continue;
            }
            let Some(window) = self.app.get_webview_window(label) else {
                continue;
            };
            if label.as_str() == "main" {
                // Force a fresh Win32 style update so the player is raised
                // above widgets that were restored just before it.
                let _ = window.set_always_on_top(false);
            }
            let _ = window.set_always_on_top(true);
        }
        flush_window_messages(&self.app);
    }
}

/// Number of native dialogs started from the frontend. The guard is created
/// for the first dialog and released when the last one finishes.
static DIALOG_COUNT: AtomicUsize = AtomicUsize::new(0);
static DIALOG_GUARD: std::sync::Mutex<Option<TopmostGuard>> = std::sync::Mutex::new(None);

/// Suspend topmost before a dialog opened from the webview. Pairs with
/// [`end_native_dialog`]; always call it in a `finally` block.
// `async` keeps this off the main thread: `is_always_on_top()` dispatches to
// the event loop and blocks for the result, which would deadlock on main.
#[tauri::command]
pub async fn begin_native_dialog(app: AppHandle) {
    if DIALOG_COUNT.fetch_add(1, Ordering::SeqCst) == 0 {
        let mut guard = DIALOG_GUARD.lock().unwrap();
        *guard = Some(TopmostGuard::suspend(&app));
    }
}

/// Restore the previous window z-order after a frontend dialog closes.
#[tauri::command]
pub async fn end_native_dialog() {
    let previous = DIALOG_COUNT.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
        Some(count.saturating_sub(1))
    });
    // Only the last dialog releases the guard.
    if !matches!(previous, Ok(1)) {
        return;
    }
    // Drop outside the mutex lock in case the window calls block briefly.
    let guard = DIALOG_GUARD.lock().unwrap().take();
    drop(guard);
}
