use crate::entities::brew_info_response::BrewInfoResponse;
use crate::entities::mas_app::MasApp;
use anyhow::Result;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::process::Command;

/// Formats a duration in seconds into a human-readable "time ago" string
pub fn format_time_ago(seconds: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY; // Approximate
    const YEAR: u64 = 365 * DAY; // Approximate

    if seconds < MINUTE {
        "just now".to_string()
    } else if seconds < HOUR {
        let minutes = seconds / MINUTE;
        if minutes == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", minutes)
        }
    } else if seconds < DAY {
        let hours = seconds / HOUR;
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    } else if seconds < WEEK {
        let days = seconds / DAY;
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if seconds < MONTH {
        let weeks = seconds / WEEK;
        if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{} weeks ago", weeks)
        }
    } else if seconds < YEAR {
        let months = seconds / MONTH;
        if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        }
    } else {
        let years = seconds / YEAR;
        if years == 1 {
            "1 year ago".to_string()
        } else {
            format!("{} years ago", years)
        }
    }
}

/// Compare two Homebrew version strings, considering revision suffixes (_X)
/// Returns Ordering::Less if a < b, Ordering::Equal if a == b, Ordering::Greater if a > b
pub fn compare_homebrew_versions(a: &str, b: &str) -> Ordering {
    // Split version and revision parts
    let (a_base, a_rev) = split_version_revision(a);
    let (b_base, b_rev) = split_version_revision(b);

    // First compare base versions
    let base_cmp = compare_version_strings(&a_base, &b_base);
    if base_cmp != Ordering::Equal {
        return base_cmp;
    }

    // If base versions are equal, compare revision numbers
    a_rev.cmp(&b_rev)
}

/// Split a version string into base version and revision number
/// e.g., "76.1_2" -> ("76.1", 2), "3.2.4" -> ("3.2.4", 0)
fn split_version_revision(version: &str) -> (String, u32) {
    if let Some(underscore_pos) = version.rfind('_') {
        let base = version[..underscore_pos].to_string();
        let revision_str = &version[underscore_pos + 1..];
        let revision = revision_str.parse::<u32>().unwrap_or(0);
        (base, revision)
    } else {
        (version.to_string(), 0)
    }
}

/// Compare two version strings numerically (e.g., "3.2.4" vs "3.10.1")
fn compare_version_strings(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let b_parts: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();

    let max_len = a_parts.len().max(b_parts.len());

    for i in 0..max_len {
        let a_part = a_parts.get(i).unwrap_or(&0);
        let b_part = b_parts.get(i).unwrap_or(&0);

        match a_part.cmp(b_part) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    Ordering::Equal
}

/// Helper functions for calling brew commands
pub fn brew_update() -> Result<()> {
    let output = Command::new("brew").arg("update").output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("brew update command failed"));
    }

    Ok(())
}

pub fn brew_info_all_installed() -> Result<BrewInfoResponse> {
    let output = Command::new("brew")
        .args(["info", "--json=v2", "--installed"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "brew info --json=v2 --installed command failed"
        ));
    }

    let output_str = String::from_utf8(output.stdout)?;
    let response: BrewInfoResponse = serde_json::from_str(&output_str)?;
    Ok(response)
}

/// Returns true if the `mas` CLI is available on the system
pub fn mas_is_installed() -> bool {
    Command::new("mas")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns all apps installed via the Mac App Store as reported by `mas list`,
/// enriched with App Store URLs and outdated status.
pub fn mas_list_installed() -> Result<Vec<MasApp>> {
    let output = Command::new("mas").arg("list").output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("mas list command failed"));
    }

    let text = String::from_utf8(output.stdout)?;
    let mut apps: Vec<MasApp> = text.lines().filter_map(parse_mas_line).collect();

    // `mas outdated` can be empty on some systems even when `mas info` shows a
    // newer store version. Use `mas info` as the source of truth for each app,
    // and keep `mas outdated` as an additional signal.
    let outdated_map = mas_outdated_apps().unwrap_or_default();
    for app in &mut apps {
        let version_from_outdated = outdated_map.get(&app.id).cloned().flatten();
        let version_from_info = mas_info_version(app.id).ok().flatten();

        app.available_version = version_from_outdated.or(version_from_info);

        let outdated_by_id = outdated_map.contains_key(&app.id);
        let outdated_by_version = app
            .available_version
            .as_deref()
            .map(|available| compare_homebrew_versions(&app.version, available) == Ordering::Less)
            .unwrap_or(false);

        app.outdated = outdated_by_id || outdated_by_version;
    }

    Ok(apps)
}

/// Returns a map of outdated app IDs to their available App Store version
/// (if the version could be parsed from `mas outdated` output).
///
/// `mas outdated` typically outputs lines in one of two formats:
///   `<id>  <name>  (<installed> -> <available>)`
///   `<id>  <name>`
fn mas_outdated_apps() -> Result<HashMap<u32, Option<String>>> {
    let output = Command::new("mas").arg("outdated").output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("mas outdated command failed"));
    }

    let text = String::from_utf8(output.stdout)?;
    let map = text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (id_str, rest) = line.split_once(char::is_whitespace)?;
            let id = id_str.trim().parse::<u32>().ok()?;

            // Try to extract the available version from " -> X.Y.Z)" at the end
            let available = parse_outdated_available_version(rest);
            Some((id, available))
        })
        .collect();

    Ok(map)
}

/// Parses the available version out of the trailing portion of a `mas outdated` line.
/// Handles formats like `AppName (1.0 -> 2.0)` or `AppName  1.0  2.0`.
fn parse_outdated_available_version(rest: &str) -> Option<String> {
    // Format: "App Name (installed -> available)"
    if let Some(arrow_pos) = rest.find("->") {
        let after_arrow = rest[arrow_pos + 2..].trim();
        // Strip a trailing ')'
        let version_str = after_arrow.trim_end_matches(')').trim();
        if !version_str.is_empty() && version_str.chars().any(|c| c.is_ascii_digit()) {
            return Some(version_str.to_string());
        }
    }

    // Format: last whitespace-separated token that looks like a version number
    let last = rest.split_whitespace().last()?;
    if last.chars().any(|c| c.is_ascii_digit()) && last.contains('.') {
        return Some(last.to_string());
    }

    None
}

/// Returns the App Store version for a given app ID from `mas info`.
fn mas_info_version(id: u32) -> Result<Option<String>> {
    let output = Command::new("mas")
        .args(["info", &id.to_string()])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("mas info command failed for {}", id));
    }

    let text = String::from_utf8(output.stdout)?;
    Ok(parse_mas_info_version(&text))
}

/// Parses `mas info` output and extracts the reported App Store version.
fn parse_mas_info_version(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Version") {
            continue;
        }

        let candidate = trimmed.split_whitespace().last()?;
        if candidate.chars().any(|c| c.is_ascii_digit()) {
            return Some(candidate.to_string());
        }
    }

    None
}

/// Uninstalls a Mac App Store app by numeric ID using `mas uninstall`.
pub fn mas_uninstall(id: u32) -> Result<()> {
    let output = Command::new("mas")
        .args(["uninstall", &id.to_string()])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("mas uninstall failed: {}", err));
    }

    Ok(())
}

/// Updates a Mac App Store app by numeric ID using `mas update`.
///
/// `mas update` triggers an App Store download which may complete asynchronously.
/// Returns `Ok(output_text)` so callers can surface what the command actually reported.
pub fn mas_upgrade(id: u32) -> Result<String> {
    let output = Command::new("mas")
        .args(["update", &id.to_string()])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(anyhow::anyhow!("mas update failed: {}", detail));
    }

    // Return whichever stream has content so the UI can show it.
    Ok(if !stdout.is_empty() { stdout } else { stderr })
}

/// Parses a single line from `mas list` output into a MasApp
/// Format: `1234567890  App Name  (1.2.3)`
fn parse_mas_line(line: &str) -> Option<MasApp> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let (id_str, rest) = line.split_once(char::is_whitespace)?;
    let id: u32 = id_str.trim().parse().ok()?;
    let rest = rest.trim();

    // Version is the last parenthesised token: `(1.2.3)`
    let version_start = rest.rfind('(')?;
    let version_end = rest.rfind(')')?;
    if version_end <= version_start {
        return None;
    }
    let version = rest[version_start + 1..version_end].trim().to_string();
    let name = rest[..version_start].trim().to_string();

    if name.is_empty() {
        return None;
    }

    Some(MasApp {
        id,
        name,
        version,
        available_version: None,
        outdated: false,
    })
}

/// Opens the GitHub issues page in the default browser
pub fn open_github_issues() -> Result<()> {
    webbrowser::open("https://github.com/cosmincatalin/brew-explorer/issues")
        .map_err(|e| anyhow::anyhow!("Failed to open browser: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_homebrew_versions_with_revisions() {
        // Test revision comparison
        assert_eq!(compare_homebrew_versions("76.1", "76.1_2"), Ordering::Less);
        assert_eq!(
            compare_homebrew_versions("76.1_2", "76.1"),
            Ordering::Greater
        );
        assert_eq!(
            compare_homebrew_versions("3.2.4", "3.2.4_4"),
            Ordering::Less
        );
        assert_eq!(
            compare_homebrew_versions("3.2.4_4", "3.2.4"),
            Ordering::Greater
        );

        // Test equal versions
        assert_eq!(compare_homebrew_versions("76.1", "76.1"), Ordering::Equal);
        assert_eq!(
            compare_homebrew_versions("76.1_2", "76.1_2"),
            Ordering::Equal
        );

        // Test different base versions
        assert_eq!(compare_homebrew_versions("76.1", "76.2"), Ordering::Less);
        assert_eq!(compare_homebrew_versions("76.2", "76.1"), Ordering::Greater);

        // Test mixed scenarios
        assert_eq!(compare_homebrew_versions("76.1_5", "76.2"), Ordering::Less);
        assert_eq!(
            compare_homebrew_versions("76.2", "76.1_5"),
            Ordering::Greater
        );
    }

    #[test]
    fn test_split_version_revision() {
        assert_eq!(split_version_revision("76.1"), ("76.1".to_string(), 0));
        assert_eq!(split_version_revision("76.1_2"), ("76.1".to_string(), 2));
        assert_eq!(split_version_revision("3.2.4_4"), ("3.2.4".to_string(), 4));
        assert_eq!(split_version_revision("1.0.0"), ("1.0.0".to_string(), 0));
    }

    #[test]
    fn test_compare_version_strings() {
        assert_eq!(compare_version_strings("3.2.4", "3.2.4"), Ordering::Equal);
        assert_eq!(compare_version_strings("3.2.3", "3.2.4"), Ordering::Less);
        assert_eq!(compare_version_strings("3.2.4", "3.2.3"), Ordering::Greater);
        assert_eq!(
            compare_version_strings("3.10.1", "3.2.4"),
            Ordering::Greater
        );
        assert_eq!(compare_version_strings("3.2.4", "3.10.1"), Ordering::Less);
    }

    #[test]
    fn test_parse_mas_info_version() {
        let info_output = "\
App WhatsApp Messenger\n\
Version 26.19.75\n\
Price Free\n\
";

        assert_eq!(
            parse_mas_info_version(info_output),
            Some("26.19.75".to_string())
        );
    }

    #[test]
    fn test_parse_mas_info_version_with_colon() {
        let info_output = "App: WhatsApp\nVersion: 26.19.75\n";
        assert_eq!(
            parse_mas_info_version(info_output),
            Some("26.19.75".to_string())
        );
    }

    #[test]
    fn test_parse_mas_info_version_with_decorative_glyphs() {
        let info_output = "App ▁▁▁▁▁▁▁▁ WhatsApp Messenger\nVersion ▁▁▁▁ 26.19.75\n";
        assert_eq!(
            parse_mas_info_version(info_output),
            Some("26.19.75".to_string())
        );
    }

    #[test]
    fn test_parse_outdated_available_version_arrow_format() {
        // Format: "AppName (installed -> available)"
        let rest = "WhatsApp (26.18.72 -> 26.19.75)";
        assert_eq!(
            parse_outdated_available_version(rest),
            Some("26.19.75".to_string())
        );
    }

    #[test]
    fn test_parse_outdated_available_version_no_version() {
        // Format: "AppName" only — no version info
        let rest = "WhatsApp";
        assert_eq!(parse_outdated_available_version(rest), None);
    }

    #[test]
    fn test_parse_outdated_available_version_two_tokens() {
        // Format: "AppName  1.0  2.0" — last token is available version
        let rest = "WhatsApp  26.18.72  26.19.75";
        assert_eq!(
            parse_outdated_available_version(rest),
            Some("26.19.75".to_string())
        );
    }
}
