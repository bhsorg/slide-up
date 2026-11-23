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

