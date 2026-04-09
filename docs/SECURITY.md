# Security Model

## Process Execution

All child process spawning goes through `ProcessExecutor::spawn()`.

**Rules enforced at the trait boundary:**

| Rule | Implementation |
|---|---|
| No shell execution | `Command::new(exe)` with `args` array — never `sh -c` |
| Allowlist | `executable.canonicalize()` checked against `allowed_executables()` before spawn |
| No path interpolation | User input only in `args: &[&str]`, never concatenated into executable |
| No process leaks | `kill_on_drop(true)` on every spawned command |

**Current allowlist:** `/opt/homebrew/bin/wine` only.

## Tauri Command Boundary

Input validation at every command:

- `app_id: u32` — typed, cannot be a path or shell metachar
- `install_path` / `exe_path` — checked for `..` (path traversal) before use
- `exe_path` — must be absolute and end with `.exe`

## Credentials

- Steam API key: stored in macOS Keychain (TODO: `security-framework` crate)
- Steam ID: stored in `UserDefaults` (not sensitive)
- SteamCMD session: cached by SteamCMD itself in its install dir

## Threat Model

| Threat | Mitigation |
|---|---|
| Shell injection via username/password | Args passed as array, no shell |
| Path traversal in install_path | `..` component check in commands |
| Unauthorized executables | ProcessExecutor allowlist |
| Credential leakage | Keychain storage (not env vars or files) |
