# mac-haze-app Architecture

## Overview

Tauri v2 desktop app (macOS). Rust backend + Nuxt/Vue frontend (via `mac-haze-web` submodule).

## Process Execution: 2-Layer Security Model

### Layer 1 — `ProcessExecutor` trait (`src/process/`)

Raw process abstraction. Security contract:
- **No `sh -c` string execution** — ever.
- Executable path is canonicalized and checked against an allowlist before any OS call.
- User input goes only into `args: &[&str]`, never into the executable path.
- `kill_on_drop(true)` prevents process leaks.

```
ProcessExecutor (trait)
├── TokioProcessExecutor   ← production impl
└── MockProcessExecutor    ← test impl (via mockall)
```

### Layer 2 — `SteamCmdClient` trait (`src/steamcmd/`)

SteamCMD-specific operations. Uses `ProcessExecutor` (injected).
Parses SteamCMD stdout into typed `SteamCmdEvent`s via `parser.rs`.

```
SteamCmdClient (trait)
├── RealSteamCmdClient     ← production impl
└── MockSteamCmdClient     ← test impl (via mockall::automock)
```

### Tauri Commands (`src/commands/`)

Input boundary. Validates types and paths before calling Layer 2.
Path traversal blocked at command boundary.

## Onboarding Saga (`src/onboarding/`)

Linear saga with compensation. App is locked until all steps complete.

```
Initial
  → SteamCmdLoginPending
  → SteamGuardPending?       (if 2FA required)
  → ApiKeyPending
  → CrossValidationPending   (GetOwnedGames cross-check)
  → Complete
```

Compensation: any failure rolls back all completed steps → `Initial`.

## App State

```rust
AppState {
    steamcmd:   Arc<dyn SteamCmdClient>,
    onboarding: Arc<dyn OnboardingSaga>,
    executor:   Arc<dyn ProcessExecutor>,
    wine_path:  PathBuf,   // /opt/homebrew/bin/wine
    wine_prefix: PathBuf,  // ~/wine-steam
    steamcmd_exe: PathBuf, // ~/wine-steam/drive_c/steamcmd/steamcmd.exe
}
```

## Tauri Event Flow

```
Frontend invoke()  →  Tauri command  →  Saga/Client
                   ←  app.emit()     ←  on state change
```

Events: `onboarding_state`, `onboarding_steamguard`, `onboarding_complete`,
`download_progress`, `download_complete`, `download_failed`
