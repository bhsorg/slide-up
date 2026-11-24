use crate::models::AppSettings;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> crate::error::Result<()> {
    state.save_settings(&settings)
}

#[tauri::command]
pub async fn reset_settings(state: State<'_, AppState>) -> crate::error::Result<AppSettings> {
    let settings = AppSettings::default();
    state.save_settings(&settings)?;
    Ok(settings)
}
