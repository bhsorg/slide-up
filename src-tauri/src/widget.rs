//! Detachable always-on-top desktop player widget.
//!
//! The widget is a regular Tauri webview window with `create: false`, so it is
//! only built on first use and reused afterwards. Its video preview borrows the
//! running mpv surface instead of starting a second player instance.

use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::state::AppState;

pub const LABEL: &str = "player-widget";
pub const VIDEO_LABEL: &str = "video-widget";
pub const PLAYLIST_LABEL: &str = "playlist-widget";

const RIGHT_MARGIN: i32 = 24;
const BOTTOM_MARGIN: i32 = 64;

/// State shared between the command thread (settings toggle) and the main
/// thread (delayed window creation). Without this, a fast off/on toggle can
/// race a pending create and leave the widget hidden while its setting says on.
#[derive(Default)]
struct WidgetState {
    pending_create: bool,
    visible: bool,
}

fn widget_state() -> &'static Mutex<WidgetState> {
    static STATE: OnceLock<Mutex<WidgetState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(WidgetState::default()))
}

/// Make the widget match its persisted setting, creating or hiding it.
///
/// The widget stays alive whenever the setting is on. While the full window is
/// open it simply drops behind it in z-order instead of covering the player.
pub fn sync(app: &AppHandle, enabled: bool) {
    if !enabled {
        hide(app);
    } else {
        show(app);
        apply_z_order(app);
    }
}

/// Show the widget when its setting is on, regardless of the main window.
/// Used right after hiding/minimizing the main window, when its visibility may
/// not have settled yet for an `is_visible()` check.
pub fn show_if_enabled(app: &AppHandle) {
    let enabled = widget_enabled(app);
    if enabled {
        show(app);
        set_topmost(app, true);
        set_main_topmost(app, main_window_visible(app));
    } else {
        hide(app);
        set_main_topmost(app, false);
    }
}

/// Re-apply the current setting after the main window's visibility changes.
pub fn sync_with_main_window(app: &AppHandle) {
    sync(app, widget_enabled(app));
}

fn widget_enabled(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| state.settings.lock().ok().map(|settings| settings.player_widget))
        .unwrap_or(false)
}

/// Keep the widget above other apps at all times. While the full player is
/// visible, also raise it above the widget so only the player covers it.
/// Tauri window commands must run on the main thread.
fn apply_z_order(app: &AppHandle) {
    let main_visible = main_window_visible(app);
    let thread_app = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        set_topmost(&thread_app, true);
        set_main_topmost(&thread_app, main_visible);
    }) {
        tracing::warn!("Could not update widget z-order: {error}");
    }
}

fn set_topmost(app: &AppHandle, topmost: bool) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_always_on_top(topmost);
    }
}

/// Raising the main window after the widget keeps the player on top of its own
/// widget while both are always-on-top against other applications.
fn set_main_topmost(app: &AppHandle, topmost: bool) {
    if let Some(window) = app.get_webview_window("main") {
        if topmost {
            // Tauri skips the Win32 call when the style is already set, so
            // toggle it to guarantee the player is raised above a new widget.
            let _ = window.set_always_on_top(false);
        }
        let _ = window.set_always_on_top(topmost);
    }
}

fn main_window_visible(app: &AppHandle) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false)
}

/// Show the widget, creating it on the main thread on first use.
pub fn show(app: &AppHandle) {
    let queue_create = {
        let mut state = widget_state().lock().unwrap();
        state.visible = true;
        if state.pending_create || app.get_webview_window(LABEL).is_some() {
            false
        } else {
            state.pending_create = true;
            true
        }
    };

    if queue_create {
        let thread_app = app.clone();
        if let Err(error) = app.run_on_main_thread(move || {
            // A toggle-off or shutdown can happen while this is queued.
            let still_wanted = widget_state().lock().unwrap().visible;
            if !still_wanted {
                widget_state().lock().unwrap().pending_create = false;
                return;
            }
            if let Err(error) = create(&thread_app) {
                widget_state().lock().unwrap().pending_create = false;
                tracing::warn!("Could not create {LABEL}: {error}");
                return;
            }
            widget_state().lock().unwrap().pending_create = false;
            if widget_state().lock().unwrap().visible {
                after_show(&thread_app);
            }
        }) {
            widget_state().lock().unwrap().pending_create = false;
            tracing::warn!("Could not schedule {LABEL}: {error}");
        }
        return;
    }

    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.show();
    }
    // Do not re-focus the main window here: a Focused event would call back
    // into this sync path. Focus is only needed right after creation.
    queue_dock(app);
}

fn after_show(app: &AppHandle) {
    queue_dock(app);
    focus_main(app);
}

/// Hide the widget instead of destroying it. Recreating a WebView2 window on
/// every toggle causes a visible CPU/RAM spike, so the window is reused.
pub fn hide(app: &AppHandle) {
    widget_state().lock().unwrap().visible = false;

    // The webview may already have revealed itself while creation was pending,
    // so always hide an existing window, not just a settled one.
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}

/// Destroy the widget window during application shutdown.
///
/// `hide()` keeps the WebView2 window alive. If the process then exits,
/// Chromium cannot unregister its `Chrome_WidgetWin_0` window class (Windows
/// error 1412, ERROR_CLASS_HAS_WINDOWS). Destroying it lets shutdown finish
/// cleanly.
pub fn destroy(app: &AppHandle) {
    {
        let mut state = widget_state().lock().unwrap();
        state.visible = false;
        state.pending_create = false;
    }

    // This runs on the main thread already (window close / tray / taskbar), so
    // return the borrowed HWND to the main window *before* the widget is
    // destroyed. Queueing this used to let widget destruction race the
    // reparent, killing mpv's render surface and hanging shutdown.
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(error) = state.mpv.set_widget_video(app, false) {
            tracing::warn!("Could not return the video surface on shutdown: {error}");
        }
    }

    for label in [LABEL, VIDEO_LABEL, PLAYLIST_LABEL] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.destroy();
        }
    }
}

fn create(app: &AppHandle) -> tauri::Result<()> {
    // Geometry and window behavior live in tauri.conf.json with
    // `create: false`, so this is the single source of truth. The matching Vue
    // root reveals the window once it has mounted to avoid a blank flash.
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == LABEL)
        .ok_or(tauri::Error::WindowNotFound)?;

    let window = WebviewWindowBuilder::from_config(app, config)?.build()?;
    // Keep the widget above other apps. The main window is raised afterwards
    // so it still covers the widget while the full player is open.
    let _ = window.set_always_on_top(true);
    set_main_topmost(app, main_window_visible(app));

    #[cfg(windows)]
    {
        // `focus: false` only covers creation. Showing or dragging an
        // always-on-top widget can activate it later. WS_EX_NOACTIVATE keeps
        // the main window as the foreground window so its custom controls stay
        // responsive, and stops the video child HWND from stealing focus when
        // it is reparented into the widget.
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
        };
        if let Ok(hwnd) = window.hwnd() {
            // Tauri uses a different `windows` crate version than this app, so
            // pass the raw pointer instead of either version's HWND newtype.
            let hwnd = hwnd.0;
            unsafe {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
            }
        }
    }

    dock(app);
    Ok(())
}

/// Dock the widget at the bottom-right of its current monitor. This only runs
/// on create/show, so a user who drags it keeps the new position until the next
/// toggle.
fn dock(app: &AppHandle) {
    let monitor = app
        .get_webview_window(LABEL)
        .and_then(|window| window.current_monitor().ok().flatten())
        .or_else(|| {
            app.get_webview_window("main")
                .and_then(|window| window.current_monitor().ok().flatten())
        });
    let Some(monitor) = monitor else {
        return;
    };

    let size = monitor.size();
    let origin = monitor.position();
    let scale = monitor.scale_factor();
    let right = origin.x + size.width as i32;
    let bottom = origin.y + size.height as i32 - BOTTOM_MARGIN;

    let (width, height) = app
        .get_webview_window(LABEL)
        .and_then(|window| window.inner_size().ok())
        .map(|size| (size.width as f64, size.height as f64))
        .or_else(|| {
            app.config()
                .app
                .windows
                .iter()
                .find(|config| config.label == LABEL)
                .map(|config| (config.width as f64 * scale, config.height as f64 * scale))
        })
        .unwrap_or((320.0 * scale, 156.0 * scale));

    let x = right - width.round() as i32 - RIGHT_MARGIN;
    let y = bottom - height.round() as i32;
    let (x, y) = remembered_position(app).unwrap_or((x, y));
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_position(tauri::Position::Physical(
            tauri::PhysicalPosition::new(x, y),
        ));
    }
}

/// Use the user's last dragged position, clamped inside the monitor that
/// contains it. Falls back to docking when the monitor layout has changed.
fn remembered_position(app: &AppHandle) -> Option<(i32, i32)> {
    let state = app.try_state::<AppState>()?;
    let (x, y) = {
        let settings = state.settings.lock().ok()?;
        (settings.player_widget_x? as i32, settings.player_widget_y? as i32)
    };
    let size = app.get_webview_window(LABEL)?.inner_size().ok()?;
    let monitor = app.available_monitors().ok()?.into_iter().find(|monitor| {
        let origin = monitor.position();
        let bounds = monitor.size();
        x >= origin.x
            && y >= origin.y
            && x < origin.x + bounds.width as i32
            && y < origin.y + bounds.height as i32
    })?;
    let area = monitor.work_area();
    let max_x = area.position.x + area.size.width as i32 - size.width as i32;
    let max_y = area.position.y + area.size.height as i32 - size.height as i32;
    Some((
        x.clamp(area.position.x, max_x.max(area.position.x)),
        y.clamp(area.position.y, max_y.max(area.position.y)),
    ))
}

fn queue_dock(app: &AppHandle) {
    let thread_app = app.clone();
    if let Err(error) = app.run_on_main_thread(move || dock(&thread_app)) {
        tracing::warn!("Could not schedule widget docking: {error}");
    }
}

/// Reparent the shared mpv surface on the main thread. Tauri window handles
/// and Win32 HWND operations should not be performed from a command thread.
/// Keep the main window active so its minimize/maximize/close buttons remain
/// responsive when the widget is shown or restored.
fn focus_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // A hidden main window (start-minimized / minimized-to-tray) cannot take
        // focus; trying to do so can stall the widget creation path.
        if window.is_visible().unwrap_or(false) {
            let _ = window.set_focus();
        }
    }
}
