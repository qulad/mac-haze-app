# mac-haze-app

Tauri v2 backend for [mac-haze](https://github.com/qulad/mac-haze).

Runs SteamCMD under Wine to download and launch Windows-only Steam games on macOS.

## Docs

- [Architecture](docs/ARCHITECTURE.md)
- [Security Model](docs/SECURITY.md)
- [Testing](docs/TESTING.md)
- [SteamCMD Reference](docs/STEAMCMD.md)

## Requirements

- macOS (Apple Silicon or Intel)
- [Homebrew](https://brew.sh)
- Wine CrossOver (installed automatically on first launch)

## Development

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install pnpm
npm install -g pnpm

# Install dependencies
pnpm install

# Run tests
cd src-tauri && cargo test

# Start dev server
pnpm tauri dev
```

## Project Structure

```
src-tauri/src/
├── process/       Layer 1 — secure process executor (ProcessExecutor trait)
├── steamcmd/      Layer 2 — SteamCMD client (SteamCmdClient trait)
├── onboarding/    Onboarding saga state machine
├── commands/      Tauri commands (input validation boundary)
└── lib.rs         App entry point + state wiring
```
