use tauri::{AppHandle, Emitter, State};

use crate::onboarding::{OnboardingError, OnboardingState};
use crate::AppState;

#[tauri::command]
pub async fn onboarding_login(
    username: String,
    password: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<OnboardingState, String> {
    let result = state.onboarding.login(&username, &password).await;

    match result {
        Ok(new_state) => {
            let _ = app.emit("onboarding_state", &new_state);
            Ok(new_state)
        }
        Err(OnboardingError::SteamGuardRequired) => {
            let s = state.onboarding.current_state();
            let _ = app.emit("onboarding_steamguard", &s);
            Err("STEAM_GUARD_REQUIRED".into())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn onboarding_submit_guard(
    code: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<OnboardingState, String> {
    state
        .onboarding
        .submit_steam_guard(&code)
        .await
        .map(|s| {
            let _ = app.emit("onboarding_state", &s);
            s
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn onboarding_validate_api_key(
    api_key: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<OnboardingState, String> {
    let validated = state
        .onboarding
        .validate_api_key(&api_key)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("onboarding_state", &validated);

    // Immediately run cross-validation
    let cross = state
        .onboarding
        .cross_validate()
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("onboarding_cross_ok", &cross);

    // Persist
    let final_state = state
        .onboarding
        .persist()
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("onboarding_complete", &final_state);
    Ok(final_state)
}

#[tauri::command]
pub async fn onboarding_status(state: State<'_, AppState>) -> Result<OnboardingState, String> {
    Ok(state.onboarding.current_state())
}
