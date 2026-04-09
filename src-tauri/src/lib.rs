use std::path::PathBuf;
use std::sync::Arc;

mod commands;
mod onboarding;
mod process;
mod steamcmd;

use onboarding::{OnboardingSaga, RealOnboardingSaga};
use process::{ProcessExecutor, TokioProcessExecutor};
use steamcmd::{RealSteamCmdClient, SteamCmdClient};

// ─── App State ─────────────────────────────────────────────────────────────

pub struct AppState {
    pub steamcmd: Arc<dyn SteamCmdClient>,
    pub onboarding: Arc<dyn OnboardingSaga>,
    pub executor: Arc<dyn ProcessExecutor>,
    pub wine_path: PathBuf,
    pub wine_prefix: PathBuf,
    pub steamcmd_exe: PathBuf,
}

// ─── Entry Point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let wine_path = PathBuf::from("/opt/homebrew/bin/wine");
    let wine_prefix = dirs::home_dir()
        .unwrap_or_default()
        .join("wine-steam");
    let steamcmd_exe = wine_prefix
        .join("drive_c/steamcmd/steamcmd.exe");

    // Allowlist: only wine is permitted to be spawned
    let executor: Arc<dyn ProcessExecutor> = Arc::new(TokioProcessExecutor::new(vec![
        wine_path.clone(),
    ]));

    let steamcmd: Arc<dyn SteamCmdClient> = Arc::new(RealSteamCmdClient::new(
        executor.clone(),
        wine_path.clone(),
        steamcmd_exe.clone(),
    ));

    let onboarding: Arc<dyn OnboardingSaga> =
        Arc::new(RealOnboardingSaga::new(steamcmd.clone()));

    let state = AppState {
        steamcmd,
        onboarding,
        executor,
        wine_path,
        wine_prefix,
        steamcmd_exe,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::onboarding::onboarding_login,
            commands::onboarding::onboarding_submit_guard,
            commands::onboarding::onboarding_validate_api_key,
            commands::onboarding::onboarding_status,
            commands::steamcmd::download_game,
            commands::steamcmd::list_owned_apps,
            commands::wine::launch_game,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
