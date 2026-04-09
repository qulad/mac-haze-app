# SteamCMD Reference

## Relevant Commands

| Command | What it does |
|---|---|
| `+login <user> <pass>` | Authenticate. May trigger Steam Guard prompt. |
| `+licenses_print` | List all owned package licenses (no API key needed) |
| `+app_info_update 1` | Refresh app metadata cache from Steam |
| `+app_info_print <appid>` | Metadata for a specific app (name, depots, etc.) |
| `+app_update <id> validate` | Download or update a game |
| `+force_install_dir <path>` | Set install directory for next `app_update` |
| `+app_status <id>` | Check if app is installed and at what version |
| `+quit` | Exit SteamCMD |

## stdout Parsing (`parser.rs`)

| Pattern | Event |
|---|---|
| `"password:"` or `"Steam>"` | `LoginPrompt` |
| `"Two-factor code:"` or `"Steam Guard code:"` | `SteamGuardPrompt` |
| `"Logged in OK"` | `LoggedIn { steam_id }` |
| `"progress: 45.23 (12345678 / 27340800)"` | `DownloadProgress { percent, downloaded_bytes, total_bytes }` |
| `"Success!"` or `"fully installed"` | `Success` |
| `"ERROR!"` or `"FAILED"` or `"Invalid Password"` | `Error(String)` |

## Session Persistence

After first login, SteamCMD writes session data to its config dir.
Subsequent `+login <username>` calls (no password) reuse the cached session silently.

## Running under Wine

```bash
wine /path/to/steamcmd.exe +login username +app_update 550 validate +quit
```

`WINEPREFIX` env var controls which Wine prefix to use.
