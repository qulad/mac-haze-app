# Testing

## Test Pyramid

```
[E2E — Playwright]          ~10 tests   mac-haze-web/tests/e2e/
[Integration — cargo test]  ~5 tests    #[ignore], requires real Wine+SteamCMD
[Component — Vitest]        ~30 tests   mac-haze-web
[Unit — cargo test]         ~50 tests   parser, saga, executor logic
```

## Running Tests

```bash
# All unit tests
cargo test

# Integration tests (requires Wine + SteamCMD installed)
cargo test -- --include-ignored

# Single module
cargo test steamcmd::parser
cargo test onboarding::saga
```

## TDD Cycle

1. Write failing test (`cargo test` → FAIL)
2. Write minimum implementation (`cargo test` → PASS)
3. Refactor

## Mock Strategy

| Layer | Mock | How |
|---|---|---|
| `ProcessExecutor` | `MockProcessExecutor` | `mockall::mock!` |
| `SteamCmdClient` | `MockSteamCmdClient` | `#[mockall::automock]` on trait |
| `OnboardingSaga` | `MockOnboardingSaga` | `#[mockall::automock]` on trait |

## Key Test Files

- `src/steamcmd/parser.rs` — inline `#[cfg(test)]` module, 8 tests
- `src/onboarding/saga.rs` — inline `#[cfg(test)]` module, 4 tests
- `src/process/executor.rs` — inline `#[cfg(test)]` module, 1 test
