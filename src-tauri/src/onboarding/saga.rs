use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::steamcmd::{LoginSession, SteamCmdClient, SteamCmdError};

// ─── State Machine ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum OnboardingState {
    Initial,
    SteamCmdLoginPending,
    SteamGuardPending {
        username: String,
    },
    SteamCmdLoginComplete {
        steam_id: u64,
        username: String,
    },
    ApiKeyPending {
        steam_id: u64,
        username: String,
    },
    CrossValidationPending {
        steam_id: u64,
        username: String,
        api_key: String,
    },
    Complete {
        steam_id: u64,
        username: String,
        api_key: String,
    },
    Failed {
        step: OnboardingStep,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum OnboardingStep {
    SteamCmdLogin,
    SteamGuard,
    ApiKeyValidation,
    CrossValidation,
    Persist,
}

// ─── Errors ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error, Serialize)]
pub enum OnboardingError {
    #[error("SteamCMD login failed: {0}")]
    LoginFailed(String),

    #[error("Steam Guard code required")]
    SteamGuardRequired,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Account mismatch: SteamCMD and API key belong to different accounts")]
    AccountMismatch,

    #[error("Persist failed: {0}")]
    PersistFailed(String),

    #[error("Invalid state transition: {0}")]
    InvalidState(String),
}

impl From<SteamCmdError> for OnboardingError {
    fn from(e: SteamCmdError) -> Self {
        match e {
            SteamCmdError::SteamGuardRequired => OnboardingError::SteamGuardRequired,
            SteamCmdError::LoginFailed(msg) => OnboardingError::LoginFailed(msg),
            other => OnboardingError::LoginFailed(other.to_string()),
        }
    }
}

// ─── Results ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CrossValidationResult {
    pub game_count: u32,
    pub display_name: String,
}

// ─── Trait ─────────────────────────────────────────────────────────────────

/// Onboarding Saga — must complete all steps before the app becomes usable.
///
/// Step 1: SteamCMD Login  (may pause at SteamGuard)
/// Step 2: API Key Validation
/// Step 3: Cross-Validation (proves both credentials target the same account)
/// Step 4: Persist to Keychain
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OnboardingSaga: Send + Sync + 'static {
    /// Step 1a: initiate SteamCMD login.
    /// Returns Ok(session) or Err(SteamGuardRequired).
    async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<OnboardingState, OnboardingError>;

    /// Step 1b: submit Steam Guard / 2FA code (called only after SteamGuardRequired).
    async fn submit_steam_guard(
        &self,
        code: &str,
    ) -> Result<OnboardingState, OnboardingError>;

    /// Step 2: validate the API key and advance state.
    async fn validate_api_key(
        &self,
        api_key: &str,
    ) -> Result<OnboardingState, OnboardingError>;

    /// Step 3: cross-validate that both credentials target the same Steam account.
    async fn cross_validate(&self) -> Result<CrossValidationResult, OnboardingError>;

    /// Step 4: persist credentials to Keychain and complete onboarding.
    async fn persist(&self) -> Result<OnboardingState, OnboardingError>;

    /// Compensation: roll back all completed steps (clear credentials, kill processes).
    async fn compensate(&self);

    fn current_state(&self) -> OnboardingState;
}

// ─── Real Implementation ────────────────────────────────────────────────────

pub struct RealOnboardingSaga {
    steamcmd: Arc<dyn SteamCmdClient>,
    http_client: reqwest::Client,
    state: Mutex<OnboardingState>,
}

impl RealOnboardingSaga {
    pub fn new(steamcmd: Arc<dyn SteamCmdClient>) -> Self {
        Self {
            steamcmd,
            http_client: reqwest::Client::new(),
            state: Mutex::new(OnboardingState::Initial),
        }
    }

    async fn set_state(&self, s: OnboardingState) {
        *self.state.lock().await = s;
    }
}

#[async_trait]
impl OnboardingSaga for RealOnboardingSaga {
    async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<OnboardingState, OnboardingError> {
        self.set_state(OnboardingState::SteamCmdLoginPending).await;

        match self.steamcmd.login(username, password).await {
            Ok(LoginSession { steam_id, username }) => {
                let next = OnboardingState::ApiKeyPending { steam_id, username };
                self.set_state(next.clone()).await;
                Ok(next)
            }
            Err(SteamCmdError::SteamGuardRequired) => {
                let next = OnboardingState::SteamGuardPending {
                    username: username.to_string(),
                };
                self.set_state(next.clone()).await;
                Err(OnboardingError::SteamGuardRequired)
            }
            Err(e) => {
                let next = OnboardingState::Failed {
                    step: OnboardingStep::SteamCmdLogin,
                    reason: e.to_string(),
                };
                self.set_state(next).await;
                Err(OnboardingError::LoginFailed(e.to_string()))
            }
        }
    }

    async fn submit_steam_guard(
        &self,
        code: &str,
    ) -> Result<OnboardingState, OnboardingError> {
        match self.steamcmd.submit_steam_guard(code).await {
            Ok(LoginSession { steam_id, username }) => {
                let next = OnboardingState::ApiKeyPending { steam_id, username };
                self.set_state(next.clone()).await;
                Ok(next)
            }
            Err(e) => {
                self.compensate().await;
                Err(e.into())
            }
        }
    }

    async fn validate_api_key(
        &self,
        api_key: &str,
    ) -> Result<OnboardingState, OnboardingError> {
        let (steam_id, username) = match self.current_state() {
            OnboardingState::ApiKeyPending { steam_id, username } => (steam_id, username),
            other => {
                return Err(OnboardingError::InvalidState(format!(
                    "Expected ApiKeyPending, got {:?}",
                    other
                )))
            }
        };

        // Verify the API key works at all by calling GetPlayerSummaries
        let url = format!(
            "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/\
             ?key={}&steamids={}",
            api_key, steam_id
        );

        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|_| OnboardingError::InvalidApiKey)?;

        if !resp.status().is_success() {
            return Err(OnboardingError::InvalidApiKey);
        }

        let next = OnboardingState::CrossValidationPending {
            steam_id,
            username,
            api_key: api_key.to_string(),
        };
        self.set_state(next.clone()).await;
        Ok(next)
    }

    async fn cross_validate(&self) -> Result<CrossValidationResult, OnboardingError> {
        let (steam_id, api_key) = match self.current_state() {
            OnboardingState::CrossValidationPending {
                steam_id, api_key, ..
            } => (steam_id, api_key),
            other => {
                return Err(OnboardingError::InvalidState(format!(
                    "Expected CrossValidationPending, got {:?}",
                    other
                )))
            }
        };

        // GetOwnedGames proves API key has access to this account's private library
        let url = format!(
            "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/\
             ?key={}&steamid={}&include_appinfo=0",
            api_key, steam_id
        );

        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|_| OnboardingError::AccountMismatch)?;

        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            self.compensate().await;
            return Err(OnboardingError::AccountMismatch);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|_| OnboardingError::AccountMismatch)?;

        let game_count = body["response"]["game_count"]
            .as_u64()
            .unwrap_or(0) as u32;

        Ok(CrossValidationResult {
            game_count,
            display_name: String::new(), // filled from GetPlayerSummaries if needed
        })
    }

    async fn persist(&self) -> Result<OnboardingState, OnboardingError> {
        let (steam_id, username, api_key) = match self.current_state() {
            OnboardingState::CrossValidationPending {
                steam_id,
                username,
                api_key,
            } => (steam_id, username, api_key),
            other => {
                return Err(OnboardingError::InvalidState(format!(
                    "Expected CrossValidationPending, got {:?}",
                    other
                )))
            }
        };

        // TODO: write to macOS Keychain via security-framework crate
        // For now: placeholder that succeeds
        let next = OnboardingState::Complete {
            steam_id,
            username,
            api_key,
        };
        self.set_state(next.clone()).await;
        Ok(next)
    }

    async fn compensate(&self) {
        // TODO: clear Keychain entries, kill any lingering SteamCMD processes
        self.set_state(OnboardingState::Initial).await;
    }

    fn current_state(&self) -> OnboardingState {
        self.state.try_lock().map(|g| g.clone()).unwrap_or(OnboardingState::Initial)
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steamcmd::client::MockSteamCmdClient;

    #[tokio::test]
    async fn login_transitions_to_api_key_pending_on_success() {
        let mut mock = MockSteamCmdClient::new();
        mock.expect_login()
            .returning(|username, _password| {
                Ok(LoginSession {
                    steam_id: 76561198000000001,
                    username: username.to_string(),
                })
            });

        let saga = RealOnboardingSaga::new(Arc::new(mock));
        let state = saga.login("testuser", "testpass").await.unwrap();

        assert!(matches!(
            state,
            OnboardingState::ApiKeyPending {
                steam_id: 76561198000000001,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn login_returns_steam_guard_required_and_pauses_state() {
        let mut mock = MockSteamCmdClient::new();
        mock.expect_login()
            .returning(|_, _| Err(SteamCmdError::SteamGuardRequired));

        let saga = RealOnboardingSaga::new(Arc::new(mock));
        let err = saga.login("testuser", "testpass").await.unwrap_err();

        assert!(matches!(err, OnboardingError::SteamGuardRequired));
        assert!(matches!(
            saga.current_state(),
            OnboardingState::SteamGuardPending { .. }
        ));
    }

    #[tokio::test]
    async fn login_failure_sets_failed_state() {
        let mut mock = MockSteamCmdClient::new();
        mock.expect_login()
            .returning(|_, _| Err(SteamCmdError::LoginFailed("Invalid password".into())));

        let saga = RealOnboardingSaga::new(Arc::new(mock));
        let err = saga.login("testuser", "wrongpass").await.unwrap_err();

        assert!(matches!(err, OnboardingError::LoginFailed(_)));
        assert!(matches!(
            saga.current_state(),
            OnboardingState::Failed { step: OnboardingStep::SteamCmdLogin, .. }
        ));
    }

    #[tokio::test]
    async fn compensate_resets_to_initial() {
        let mock = MockSteamCmdClient::new();
        let saga = RealOnboardingSaga::new(Arc::new(mock));
        saga.set_state(OnboardingState::SteamGuardPending {
            username: "user".into(),
        })
        .await;

        saga.compensate().await;

        assert_eq!(saga.current_state(), OnboardingState::Initial);
    }
}
