use crate::error::{AppError, Result};
use std::path::Path;

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn open_external(url: String) -> Result<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::native("Only web links can be opened from the app."));
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&url)
            .spawn()
            .map_err(|err| AppError::native(format!("Could not open the link: {err}")))?;
    }
    Ok(())
}

#[tauri::command]
pub fn show_in_folder(path: String) -> Result<()> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err(AppError::media("This file no longer exists on disk."));
    }
    #[cfg(windows)]
    {
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| path.to_path_buf())
        };
        std::process::Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|err| AppError::native(format!("Could not open the folder: {err}")))?;
    }
    Ok(())
}

/// Whether this launch must leave the main window hidden and show only the
/// desktop widget. The frontend needs this because the main window is created
/// hidden for the splash reveal on every launch; "currently hidden" alone
/// cannot distinguish an autostart from a normal launch.
#[tauri::command]
pub fn is_widget_only_launch(app: tauri::AppHandle) -> Result<bool> {
    use tauri::Manager;
    let state = app.state::<crate::state::AppState>();
    let widget_enabled = state.settings.lock().unwrap().player_widget;
    // A workspace chooser needs the main window visible.
    let multiple_workspaces = crate::workspace::list_workspaces(&state.app_db.lock().unwrap())?.len() > 1;
    Ok(!state.is_locked()
        && !multiple_workspaces
        && crate::started_from_autostart()
        && widget_enabled)
}

/// Show the desktop widget after the main window is minimized or hidden.
#[tauri::command]
pub fn show_widget_when_main_hidden(app: tauri::AppHandle) -> Result<()> {
    crate::widget::show_if_enabled(&app);
    Ok(())
}

/// Restore the full window from the desktop widget and step the widget aside.
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<()> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    crate::widget::sync_with_main_window(&app);
    Ok(())
}

/// Register or remove the per-user Windows startup entry. The OS entry is the
/// source of truth, so the setting is queried from the OS rather than mirrored
/// into our own settings file.
#[tauri::command]
pub fn set_launch_at_startup(app: tauri::AppHandle, enabled: bool) -> Result<()> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch
            .enable()
            .map_err(|err| AppError::native(format!("Could not enable startup launch: {err}")))?;
    } else {
        autolaunch
            .disable()
            .map_err(|err| AppError::native(format!("Could not disable startup launch: {err}")))?;
    }
    Ok(())
}

#[tauri::command]
pub fn is_launch_at_startup(app: tauri::AppHandle) -> Result<bool> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|err| AppError::native(format!("Could not check startup launch: {err}")))
}

#[tauri::command]
pub async fn fetch_url_text(url: String) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("SlideUp/0.1.0")
        .build()
        .map_err(|err| AppError::native(format!("Failed to build client: {err}")))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|err| AppError::native(format!("Request failed: {err}")))?;
    let text = response
        .text()
        .await
        .map_err(|err| AppError::native(format!("Failed to read text: {err}")))?;
    Ok(text)
}

