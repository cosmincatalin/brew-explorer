use serde::Deserialize;
use std::cmp::Ordering;

/// Formats a duration in seconds into a human-readable "time ago" string
fn format_time_ago(seconds: u64) -> String {
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
fn compare_homebrew_versions(a: &str, b: &str) -> Ordering {
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
    }/// Represents a Homebrew package with its metadata
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub description: String,
    pub homepage: String,
    pub current_version: String,
    pub installed_version: Option<String>,
    pub package_type: PackageType,
    pub tap: Option<String>,
    pub outdated: bool,
    pub caveats: Option<String>,
    pub installed_at: Option<u64>, // Unix timestamp
}

/// Represents the type of a Homebrew package
#[derive(Debug, Clone, PartialEq)]
pub enum PackageType {
    Formulae,
    Cask,
    Unknown,
}

/// Serde structures for parsing brew JSON output
#[derive(Debug, Deserialize)]
pub struct BrewInfoResponse {
    pub formulae: Vec<BrewFormulae>,
    pub casks: Vec<BrewCask>,
}

#[derive(Debug, Deserialize)]
pub struct BrewFormulae {
    pub name: String,
    pub tap: String,
    pub desc: String,
    pub homepage: String,
    pub versions: BrewVersions,
    pub installed: Vec<BrewInstalled>,
    pub outdated: bool,
    pub caveats: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BrewCask {
    pub token: String,
    pub tap: String,
    pub name: Vec<String>,
    pub desc: Option<String>,
    pub homepage: String,
    pub version: String,
    pub installed: Option<String>,
    pub outdated: bool,
    pub caveats: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BrewVersions {
    pub stable: Option<String>,
    pub head: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BrewInstalled {
    pub version: String,
    pub time: u64,
    pub installed_as_dependency: bool,
    pub installed_on_request: bool,
}

impl PackageInfo {
    /// Creates a new PackageInfo instance with type information
    pub fn new_with_type(
        name: String,
        description: String,
        homepage: String,
        current_version: String,
        installed_version: Option<String>,
        package_type: PackageType,
        tap: Option<String>,
    ) -> Self {
        Self {
            name,
            description,
            homepage,
            current_version,
            installed_version,
            package_type,
            tap,
            outdated: false,
            caveats: None,
            installed_at: None,
        }
    }
    /// Creates a new PackageInfo instance with all information including caveats
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_caveats(
        name: String,
        description: String,
        homepage: String,
        current_version: String,
        installed_version: Option<String>,
        package_type: PackageType,
        tap: Option<String>,
        outdated: bool,
        caveats: Option<String>,
        installed_at: Option<u64>,
    ) -> Self {
        Self {
            name,
            description,
            homepage,
            current_version,
            installed_version,
            package_type,
            tap,
            outdated,
            caveats,
            installed_at,
        }
    }

    /// Checks if the package is installed
    pub fn is_installed(&self) -> bool {
        self.installed_version.is_some()
    }

    /// Checks if the package has an update available
    pub fn has_update_available(&self) -> bool {
        match &self.installed_version {
            Some(installed) => {
                // Use version comparison that understands revisions
                match compare_homebrew_versions(installed, &self.current_version) {
                    Ordering::Less => true,  // installed < current, update available
                    Ordering::Equal | Ordering::Greater => false,  // installed >= current, no update needed
                }
            }
            None => false,
        }
    }

    /// Gets the installation status as a formatted string
    pub fn installation_status(&self) -> String {
        match &self.installed_version {
            Some(version) => {
                if version == &self.current_version {
                    format!("{} (up to date)", version)
                } else {
                    format!("{} (update available)", version)
                }
            }
            None => "Not installed".to_string(),
        }
    }
    
    /// Returns a human-readable string indicating how long ago the package was installed
    pub fn installed_ago(&self) -> Option<String> {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        if let Some(timestamp) = self.installed_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            
            if now >= timestamp {
                let diff = now - timestamp;
                return Some(format_time_ago(diff));
            }
        }
        None
    }

    /// Gets the display name with package type prefix
    pub fn get_display_name(&self) -> String {
        match self.package_type {
            PackageType::Formulae => format!("⚙️ {}", self.name),
            PackageType::Cask => format!("🍺 {}", self.name),
            PackageType::Unknown => self.name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_update_available_with_revisions() {
        // Case 1: Installed version 3.2.4_4 is newer than stable 3.2.4 - no update needed
        let package1 = PackageInfo {
            name: "httpie".to_string(),
            description: "HTTP client".to_string(),
            homepage: "https://httpie.io".to_string(),
            current_version: "3.2.4".to_string(),
            installed_version: Some("3.2.4_4".to_string()),
            package_type: PackageType::Formulae,
            tap: None,
            outdated: false,
            caveats: None,
            installed_at: Some(1696118400), // Example timestamp
        };
        assert_eq!(package1.has_update_available(), false);

        // Case 2: Installed version 3.2.4 is older than stable 3.2.5 - update available
        let package2 = PackageInfo {
            name: "httpie".to_string(),
            description: "HTTP client".to_string(),
            homepage: "https://httpie.io".to_string(),
            current_version: "3.2.5".to_string(),
            installed_version: Some("3.2.4".to_string()),
            package_type: PackageType::Formulae,
            tap: None,
            outdated: false,
            caveats: None,
            installed_at: Some(1696118400), // Example timestamp
        };
        assert_eq!(package2.has_update_available(), true);

        // Case 3: Installed version 76.1 is older than stable 76.1_2 - update available
        let package3 = PackageInfo {
            name: "somepackage".to_string(),
            description: "Some package".to_string(),
            homepage: "https://example.com".to_string(),
            current_version: "76.1_2".to_string(),
            installed_version: Some("76.1".to_string()),
            package_type: PackageType::Formulae,
            tap: None,
            outdated: false,
            caveats: None,
            installed_at: Some(1696118400), // Example timestamp
        };
        assert_eq!(package3.has_update_available(), true);
    }
}
