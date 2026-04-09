use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub enum SteamCmdEvent {
    LoginPrompt,
    SteamGuardPrompt,
    LoggedIn { steam_id: u64 },
    DownloadProgress {
        percent: u8,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    /// Emitted once per package entry from `licenses_print`.
    /// A single user may have many packages; collect all and deduplicate.
    LicenseApps { app_ids: Vec<u32> },
    Success,
    Error(String),
}

static PROGRESS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"progress: (\d+)\.\d+ \((\d+) / (\d+)\)").expect("valid regex")
});

static STEAM_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Logged in OK\s*\(?(\d{17})?\)?").expect("valid regex")
});

// "  - Apps                  : 570 440 730 "
static LICENSE_APPS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*-\s*Apps\s*:\s*([\d\s]+)").expect("valid regex")
});

pub fn parse_line(line: &str) -> Option<SteamCmdEvent> {
    if line.contains("password:") || line.starts_with("Steam>") {
        return Some(SteamCmdEvent::LoginPrompt);
    }

    if line.contains("Two-factor code:") || line.contains("Steam Guard code:") {
        return Some(SteamCmdEvent::SteamGuardPrompt);
    }

    if line.starts_with("Success!") || line.contains("fully installed") {
        return Some(SteamCmdEvent::Success);
    }

    if line.contains("ERROR!") || line.contains("FAILED") || line.contains("Invalid Password") {
        return Some(SteamCmdEvent::Error(line.to_string()));
    }

    if let Some(caps) = PROGRESS_RE.captures(line) {
        return Some(SteamCmdEvent::DownloadProgress {
            percent: caps[1].parse().unwrap_or(0),
            downloaded_bytes: caps[2].parse().unwrap_or(0),
            total_bytes: caps[3].parse().unwrap_or(0),
        });
    }

    if let Some(caps) = LICENSE_APPS_RE.captures(line) {
        let app_ids: Vec<u32> = caps[1]
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if !app_ids.is_empty() {
            return Some(SteamCmdEvent::LicenseApps { app_ids });
        }
    }

    if line.contains("Logged in OK") {
        let steam_id = STEAM_ID_RE
            .captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        return Some(SteamCmdEvent::LoggedIn { steam_id });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_steam_guard_prompt() {
        assert_eq!(
            parse_line("Two-factor code:"),
            Some(SteamCmdEvent::SteamGuardPrompt)
        );
    }

    #[test]
    fn detects_steam_guard_alternate_prompt() {
        assert_eq!(
            parse_line("Steam Guard code:"),
            Some(SteamCmdEvent::SteamGuardPrompt)
        );
    }

    #[test]
    fn detects_login_prompt() {
        assert_eq!(
            parse_line("Steam>password: "),
            Some(SteamCmdEvent::LoginPrompt)
        );
    }

    #[test]
    fn parses_download_progress() {
        let line = "Update state (0x61) downloading, progress: 45.23 (12345678 / 27340800)";
        assert_eq!(
            parse_line(line),
            Some(SteamCmdEvent::DownloadProgress {
                percent: 45,
                downloaded_bytes: 12345678,
                total_bytes: 27340800,
            })
        );
    }

    #[test]
    fn parses_progress_at_100() {
        let line = "Update state (0x61) downloading, progress: 100.00 (27340800 / 27340800)";
        assert_eq!(
            parse_line(line),
            Some(SteamCmdEvent::DownloadProgress {
                percent: 100,
                downloaded_bytes: 27340800,
                total_bytes: 27340800,
            })
        );
    }

    #[test]
    fn parses_success() {
        assert_eq!(
            parse_line("Success! App '550' fully installed."),
            Some(SteamCmdEvent::Success)
        );
    }

    #[test]
    fn parses_error() {
        let event = parse_line("ERROR! Failed to install app '550'");
        assert!(matches!(event, Some(SteamCmdEvent::Error(_))));
    }

    #[test]
    fn returns_none_for_unrelated_line() {
        assert_eq!(parse_line("Connecting anonymously to Steam Public..."), None);
    }

    #[test]
    fn parses_license_apps_line() {
        let line = " - Apps                  : 570 440 730 ";
        assert_eq!(
            parse_line(line),
            Some(SteamCmdEvent::LicenseApps {
                app_ids: vec![570, 440, 730],
            })
        );
    }

    #[test]
    fn parses_license_apps_single() {
        let line = " - Apps                  : 550 ";
        assert_eq!(
            parse_line(line),
            Some(SteamCmdEvent::LicenseApps {
                app_ids: vec![550],
            })
        );
    }

    #[test]
    fn skips_empty_apps_line() {
        // Some packages have no apps (DLC-only packages, etc.)
        assert_eq!(parse_line(" - Apps                  : "), None);
    }

    #[test]
    fn parses_logged_in() {
        assert!(matches!(
            parse_line("Logged in OK"),
            Some(SteamCmdEvent::LoggedIn { .. })
        ));
    }
}
