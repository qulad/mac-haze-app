use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::steamcmd::DownloadEvent;
use crate::AppState;

#[tauri::command]
pub async fn download_game(
    app_id: u32,
    install_path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    // Input validation: block path traversal
    let path = PathBuf::from(&install_path);
    if path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err("Invalid install path".into());
    }

    let mut rx = state
        .steamcmd
        .download_app(app_id, &path)
        .await
        .map_err(|e| e.to_string())?;

    // Stream download events to the frontend
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let event_name = match &event {
                DownloadEvent::Progress { .. } => "download_progress",
                DownloadEvent::Completed => "download_complete",
                DownloadEvent::Failed { .. } => "download_failed",
            };
            let _ = app.emit(event_name, &event);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn list_owned_apps(state: State<'_, AppState>) -> Result<Vec<u32>, String> {
    state
        .steamcmd
        .list_owned_apps()
        .await
        .map_err(|e| e.to_string())
}
