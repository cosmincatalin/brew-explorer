use anyhow::Result;
use std::cmp::Ordering;
use std::process::Command;
use crate::entities::brew_info_response::{BrewCask, BrewFormula, BrewInfoResponse};
use crate::entities::package_info::{PackageInfo, PackageType};

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
pub fn brew_info_all_installed() -> Result<BrewInfoResponse> {
    let output = Command::new("brew")
        .args(["info", "--json=v2", "--installed"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("brew info --json=v2 --installed command failed"));
    }

    let output_str = String::from_utf8(output.stdout)?;
    let response: BrewInfoResponse = serde_json::from_str(&output_str)?;
    Ok(response)
}

pub fn brew_info(package_name: &str) -> Result<BrewInfoResponse> {
    let output = Command::new("brew")
        .args(["info", "--json=v2", package_name])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("brew info command failed for {}", package_name));
    }

    let output_str = String::from_utf8(output.stdout)?;
    let response: BrewInfoResponse = serde_json::from_str(&output_str)?;
    Ok(response)
}

/// Convert a brew Formulae JSON to our PackageInfo structure
pub fn brew_formulae_to_package_info(formula: &BrewFormula) -> PackageInfo {
    let (installed_version, installed_at) = if !formula.installed.is_empty() {
        let latest_install = formula.installed
            .iter()
            .max_by_key(|install| install.time);
        match latest_install {
            Some(install) => (Some(install.version.clone()), Some(install.time)),
            None => (None, None),
        }
    } else {
        (None, None)
    };

    PackageInfo::new(
        formula.name.clone(),
        formula.desc.clone(),
        formula.homepage.clone(),
        formula.versions.stable.clone().unwrap_or_else(|| "unknown".to_string()),
        installed_version,
        PackageType::Formulae,
        Some(formula.tap.clone()),
        formula.outdated,
        formula.caveats.clone(),
        installed_at,
    )
}

/// Convert a brew Cask JSON to our PackageInfo structure
pub fn brew_cask_to_package_info(cask: &BrewCask) -> PackageInfo {
    let installed_version = cask.installed.clone();

    let description = cask.desc.clone().unwrap_or_else(|| {
        if cask.name.is_empty() {
            "No description available".to_string()
        } else {
            cask.name.join(", ")
        }
    });

    PackageInfo::new(
        cask.token.clone(),
        description,
        cask.homepage.clone(),
        cask.version.clone(),
        installed_version,
        PackageType::Cask,
        Some(format!("{} (cask)", cask.tap)),
        cask.outdated,
        cask.caveats.clone(),
        None, // Casks don't have installation timestamp in the JSON
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_homebrew_versions_with_revisions() {
        // Test revision comparison
        assert_eq!(compare_homebrew_versions("76.1", "76.1_2"), Ordering::Less);
        assert_eq!(compare_homebrew_versions("76.1_2", "76.1"), Ordering::Greater);
        assert_eq!(compare_homebrew_versions("3.2.4", "3.2.4_4"), Ordering::Less);
        assert_eq!(compare_homebrew_versions("3.2.4_4", "3.2.4"), Ordering::Greater);

        // Test equal versions
        assert_eq!(compare_homebrew_versions("76.1", "76.1"), Ordering::Equal);
        assert_eq!(compare_homebrew_versions("76.1_2", "76.1_2"), Ordering::Equal);

        // Test different base versions
        assert_eq!(compare_homebrew_versions("76.1", "76.2"), Ordering::Less);
        assert_eq!(compare_homebrew_versions("76.2", "76.1"), Ordering::Greater);

        // Test mixed scenarios
        assert_eq!(compare_homebrew_versions("76.1_5", "76.2"), Ordering::Less);
        assert_eq!(compare_homebrew_versions("76.2", "76.1_5"), Ordering::Greater);
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
        assert_eq!(compare_version_strings("3.10.1", "3.2.4"), Ordering::Greater);
        assert_eq!(compare_version_strings("3.2.4", "3.10.1"), Ordering::Less);
    }
}
