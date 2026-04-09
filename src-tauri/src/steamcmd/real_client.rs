use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::client::{DownloadEvent, LoginSession, SteamCmdClient, SteamCmdError};
use super::parser::{parse_line, SteamCmdEvent};
use crate::process::{ProcessExecutor, ProcessHandle, ProcessOutput};

pub struct RealSteamCmdClient {
    executor: Arc<dyn ProcessExecutor>,
    wine_path: PathBuf,
    steamcmd_exe: PathBuf,
    /// Active login session handle (kept alive for submit_steam_guard)
    pending_handle: Mutex<Option<ProcessHandle>>,
    /// Confirmed session after successful login
    session: Mutex<Option<LoginSession>>,
}

impl RealSteamCmdClient {
    pub fn new(
        executor: Arc<dyn ProcessExecutor>,
        wine_path: PathBuf,
        steamcmd_exe: PathBuf,
    ) -> Self {
        Self {
            executor,
            wine_path,
            steamcmd_exe,
            pending_handle: Mutex::new(None),
            session: Mutex::new(None),
        }
    }
}

#[async_trait]
impl SteamCmdClient for RealSteamCmdClient {
    async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<LoginSession, SteamCmdError> {
        let steamcmd_str = self.steamcmd_exe.to_string_lossy().to_string();
        let handle = self
            .executor
            .spawn(
                &self.wine_path,
                &[&steamcmd_str, "+login", username, password],
                &[],
                None,
            )
            .await?;

        let mut handle = handle;
        while let Some(output) = handle.output_rx.recv().await {
            let line = match output {
                ProcessOutput::Stdout(l) | ProcessOutput::Stderr(l) => l,
            };

            match parse_line(&line) {
                Some(SteamCmdEvent::SteamGuardPrompt) => {
                    *self.pending_handle.lock().await = Some(handle);
                    return Err(SteamCmdError::SteamGuardRequired);
                }
                Some(SteamCmdEvent::LoggedIn { steam_id }) => {
                    let session = LoginSession {
                        steam_id,
                        username: username.to_string(),
                    };
                    *self.session.lock().await = Some(session.clone());
                    return Ok(session);
                }
                Some(SteamCmdEvent::Error(msg)) => {
                    return Err(SteamCmdError::LoginFailed(msg));
                }
                _ => {}
            }
        }

        Err(SteamCmdError::LoginFailed("Unexpected end of output".into()))
    }

    async fn submit_steam_guard(&self, code: &str) -> Result<LoginSession, SteamCmdError> {
        let mut guard = self.pending_handle.lock().await;
        let handle = guard.as_mut().ok_or_else(|| {
            SteamCmdError::UnexpectedOutput("No pending Steam Guard session".into())
        })?;

        handle.write_line(code).await?;

        while let Some(output) = handle.output_rx.recv().await {
            let line = match output {
                ProcessOutput::Stdout(l) | ProcessOutput::Stderr(l) => l,
            };

            match parse_line(&line) {
                Some(SteamCmdEvent::LoggedIn { steam_id }) => {
                    let session = self
                        .session
                        .lock()
                        .await
                        .clone()
                        .unwrap_or_else(|| LoginSession {
                            steam_id,
                            username: String::new(),
                        });
                    *self.session.lock().await = Some(session.clone());
                    *guard = None;
                    return Ok(session);
                }
                Some(SteamCmdEvent::Error(msg)) => {
                    return Err(SteamCmdError::LoginFailed(msg));
                }
                _ => {}
            }
        }

        Err(SteamCmdError::LoginFailed("Steam Guard validation failed".into()))
    }

    async fn download_app(
        &self,
        app_id: u32,
        install_path: &Path,
    ) -> Result<mpsc::Receiver<DownloadEvent>, SteamCmdError> {
        let session = self
            .session
            .lock()
            .await
            .clone()
            .ok_or_else(|| SteamCmdError::LoginFailed("Not logged in".into()))?;

        let install_str = install_path.to_string_lossy().to_string();
        let steamcmd_str = self.steamcmd_exe.to_string_lossy().to_string();
        let app_id_str = app_id.to_string();

        let handle = self
            .executor
            .spawn(
                &self.wine_path,
                &[
                    &steamcmd_str,
                    "+login",
                    &session.username,
                    "+force_install_dir",
                    &install_str,
                    "+app_update",
                    &app_id_str,
                    "validate",
                    "+quit",
                ],
                &[],
                None,
            )
            .await?;

        let (event_tx, event_rx) = mpsc::channel::<DownloadEvent>(64);

        tokio::spawn(async move {
            let mut handle = handle;
            while let Some(output) = handle.output_rx.recv().await {
                let line = match output {
                    ProcessOutput::Stdout(l) | ProcessOutput::Stderr(l) => l,
                };

                match parse_line(&line) {
                    Some(SteamCmdEvent::DownloadProgress {
                        percent,
                        downloaded_bytes,
                        total_bytes,
                    }) => {
                        let _ = event_tx
                            .send(DownloadEvent::Progress {
                                percent,
                                downloaded_bytes,
                                total_bytes,
                            })
                            .await;
                    }
                    Some(SteamCmdEvent::Success) => {
                        let _ = event_tx.send(DownloadEvent::Completed).await;
                        break;
                    }
                    Some(SteamCmdEvent::Error(msg)) => {
                        let _ = event_tx
                            .send(DownloadEvent::Failed { reason: msg })
                            .await;
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(event_rx)
    }

    async fn list_owned_apps(&self) -> Result<Vec<u32>, SteamCmdError> {
        // TODO: parse licenses_print output
        // Placeholder returns empty list
        Ok(vec![])
    }

    async fn quit(&self) -> Result<(), SteamCmdError> {
        Ok(())
    }
}
