use super::models::{Folder, FolderSummary};
use crate::database::repositories::folders::{FolderError, FoldersRepository};
use crate::state::AppState;
use log::{error as log_error, info as log_info};

fn ui_error(context: &str, e: FolderError) -> String {
    if matches!(e, FolderError::Db(_)) {
        log_error!("{}: {}", context, e);
    }
    e.to_string()
}

#[tauri::command]
pub async fn api_list_folders(state: tauri::State<'_, AppState>) -> Result<Vec<FolderSummary>, String> {
    FoldersRepository::list_folders(state.db_manager.pool())
        .await
        .map_err(|e| ui_error("api_list_folders", e))
}

#[tauri::command]
pub async fn api_create_folder(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Folder, String> {
    FoldersRepository::create_folder(state.db_manager.pool(), &name)
        .await
        .map_err(|e| ui_error("api_create_folder", e))
}

#[tauri::command]
pub async fn api_rename_folder(
    state: tauri::State<'_, AppState>,
    folder_id: String,
    name: String,
) -> Result<Folder, String> {
    FoldersRepository::rename_folder(state.db_manager.pool(), &folder_id, &name)
        .await
        .map_err(|e| ui_error("api_rename_folder", e))
}

/// Delete a folder; its meetings are kept and become unfiled.
#[tauri::command]
pub async fn api_delete_folder(
    state: tauri::State<'_, AppState>,
    folder_id: String,
) -> Result<bool, String> {
    log_info!("api_delete_folder called for {}", folder_id);
    FoldersRepository::delete_folder(state.db_manager.pool(), &folder_id)
        .await
        .map_err(|e| ui_error("api_delete_folder", e))
}

/// File a meeting in a folder, or pass no folder to make it unfiled.
#[tauri::command]
pub async fn api_move_meeting_to_folder(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    FoldersRepository::move_meeting(state.db_manager.pool(), &meeting_id, folder_id.as_deref())
        .await
        .map_err(|e| ui_error("api_move_meeting_to_folder", e))
}
