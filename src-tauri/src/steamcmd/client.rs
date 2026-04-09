use async_trait::async_trait;
use serde::Serialize;
use std::path::Path;
use tokio::sync::mpsc;

use crate::process::ProcessError;

#[derive(Debug, thiserror::Error)]
pub enum SteamCmdError {
    #[error("Login failed: {0}")]
    LoginFailed(String),

    #[error("Steam Guard code required")]
    SteamGuardRequired,

    #[error("Process error: {0}")]
    Process(#[from] ProcessError),

    #[error("Unexpected output: {0}")]
    UnexpectedOutput(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),
}

#[derive(Debug, Clone)]
pub struct LoginSession {
    pub steam_id: u64,
    pub username: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DownloadEvent {
    Progress {
        percent: u8,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    Completed,
    Failed {
        reason: String,
    },
}

/// Layer 2: SteamCMD-specific operations.
/// Knows about SteamCMD semantics; delegates process execution to ProcessExecutor.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait SteamCmdClient: Send + Sync + 'static {
    /// Initiate login. Returns Ok if login completed without Steam Guard,
    /// or Err(SteamGuardRequired) if a code is needed.
    async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<LoginSession, SteamCmdError>;

    /// Submit a Steam Guard / 2FA code after a SteamGuardRequired error.
    async fn submit_steam_guard(&self, code: &str) -> Result<LoginSession, SteamCmdError>;

    /// Download or update an app. Returns a channel streaming DownloadEvent.
    async fn download_app(
        &self,
        app_id: u32,
        install_path: &Path,
    ) -> Result<mpsc::Receiver<DownloadEvent>, SteamCmdError>;

    /// List owned app IDs from cached licenses.
    async fn list_owned_apps(&self) -> Result<Vec<u32>, SteamCmdError>;

    async fn quit(&self) -> Result<(), SteamCmdError>;
}
