use std::path::PathBuf;

use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn launch_game(
    exe_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let exe = PathBuf::from(&exe_path);

    // Must be an absolute path and end with .exe
    if !exe.is_absolute() {
        return Err("exe_path must be absolute".into());
    }
    if exe.extension().and_then(|e| e.to_str()) != Some("exe") {
        return Err("exe_path must point to a .exe file".into());
    }
    // Block path traversal
    if exe
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err("Invalid exe path".into());
    }

    let exe_str = exe.to_string_lossy().to_string();
    let _handle = state
        .executor
        .spawn(
            &state.wine_path,
            &[&exe_str],
            &[("WINEPREFIX", state.wine_prefix.to_str().unwrap_or(""))],
            None,
        )
        .await
        .map_err(|e| e.to_string())?;

    // Handle is intentionally dropped — game runs detached
    Ok(())
}
