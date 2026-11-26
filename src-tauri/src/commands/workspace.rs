use crate::error::{AppError, Result};
use crate::state::AppState;
use crate::workspace::{self, WorkspaceCreation, WorkspaceInfo};
use tauri::State;

#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> Result<Vec<WorkspaceInfo>> {
    workspace::list_workspaces(&state.app_db.lock().unwrap())
}

#[tauri::command]
pub fn active_workspace(state: State<'_, AppState>) -> Result<Option<WorkspaceInfo>> {
    Ok(state.active_workspace())
}

#[tauri::command]
pub fn create_workspace(state: State<'_, AppState>, name: String) -> Result<WorkspaceCreation> {
    workspace::create_workspace(&state.app_db.lock().unwrap(), &state.data_dir, &name)
}

#[tauri::command]
pub fn switch_workspace(state: State<'_, AppState>, id: String) -> Result<WorkspaceInfo> {
    state.switch_workspace(&id)
}

/// Select a workspace without unlocking it, so the lock screen can ask for
/// its password before any of its data is opened.
#[tauri::command]
pub fn select_workspace(state: State<'_, AppState>, id: String) -> Result<WorkspaceInfo> {
    state.select_workspace(&id)
}

#[tauri::command]
pub fn lock_workspace(state: State<'_, AppState>) -> Result<WorkspaceInfo> {
    state.lock_workspace()
}

#[tauri::command]
pub fn unlock_workspace(state: State<'_, AppState>, id: Option<String>) -> Result<WorkspaceInfo> {
    state.unlock_workspace(id)
}

// Argon2 key derivation is deliberately expensive; keep it off the main
// thread so the window stays responsive while unlocking.
#[tauri::command]
pub async fn unlock_workspace_with_password(
    state: State<'_, AppState>,
    id: String,
    password: String,
) -> Result<WorkspaceInfo> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.unlock_with_password(&id, &password))
        .await
        .map_err(|err| AppError::native(format!("Could not unlock the workspace: {err}")))?
}

#[tauri::command]
pub async fn set_workspace_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<WorkspaceInfo> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.set_workspace_password(&password))
        .await
        .map_err(|err| AppError::native(format!("Could not set the workspace password: {err}")))?
}

#[tauri::command]
pub async fn clear_workspace_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<WorkspaceInfo> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.clear_workspace_password(&password))
        .await
        .map_err(|err| AppError::native(format!("Could not clear the workspace password: {err}")))?
}

#[tauri::command]
pub fn pending_workspace(state: State<'_, AppState>) -> Result<Option<WorkspaceInfo>> {
    let id = workspace::last_active(&state.app_db.lock().unwrap())?;
    match id {
        Some(id) => workspace::get_workspace(&state.app_db.lock().unwrap(), &id),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn rename_workspace(state: State<'_, AppState>, id: String, name: String) -> Result<()> {
    workspace::rename_workspace(&state.app_db.lock().unwrap(), &id, &name)
}

#[tauri::command]
pub fn set_workspace_avatar(state: State<'_, AppState>, path: String) -> Result<WorkspaceInfo> {
    let id = state
        .active
        .lock()
        .unwrap()
        .as_ref()
        .map(|active| active.id.clone())
        .ok_or_else(|| AppError::native("Unlock a workspace before changing its avatar."))?;
    let app_db = state.app_db.lock().unwrap();
    workspace::set_avatar(&app_db, &state.data_dir, &id, &path)?;
    workspace::get_workspace(&app_db, &id)?
        .ok_or_else(|| AppError::native("Workspace not found."))
}

#[tauri::command]
pub fn clear_workspace_avatar(state: State<'_, AppState>) -> Result<WorkspaceInfo> {
    let id = state
        .active
        .lock()
        .unwrap()
        .as_ref()
        .map(|active| active.id.clone())
        .ok_or_else(|| AppError::native("Unlock a workspace before changing its avatar."))?;
    let app_db = state.app_db.lock().unwrap();
    workspace::clear_avatar(&app_db, &state.data_dir, &id)?;
    workspace::get_workspace(&app_db, &id)?
        .ok_or_else(|| AppError::native("Workspace not found."))
}

#[tauri::command]
pub fn delete_workspace(state: State<'_, AppState>, id: String) -> Result<()> {
    let active_id = state.active.lock().unwrap().as_ref().map(|a| a.id.clone());
    if active_id.as_deref() == Some(id.as_str()) {
        return Err(AppError::native("Switch to another workspace before deleting this one."));
    }
    workspace::delete_workspace(&state.app_db.lock().unwrap(), &state.data_dir, &id)
}

#[tauri::command]
pub fn restore_workspace(state: State<'_, AppState>, recovery_key: String) -> Result<WorkspaceInfo> {
    workspace::restore_workspace(
        &state.app_db.lock().unwrap(),
        &state.data_dir,
        &recovery_key,
    )
}
