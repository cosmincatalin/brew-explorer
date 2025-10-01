use serde::Deserialize;
use std::cmp::Ordering;

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
}

/// Represents a Homebrew package with its metadata
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
    pub formulae: Vec<BrewFormula>,
    pub casks: Vec<BrewCask>,
}

#[derive(Debug, Deserialize)]
pub struct BrewFormula {
    pub name: String,
    pub full_name: String,
    pub tap: String,
    pub desc: String,
    pub homepage: String,
    pub versions: BrewVersions,
    pub installed: Vec<BrewInstalled>,
    pub linked_keg: Option<String>,
    pub outdated: bool,
}

#[derive(Debug, Deserialize)]
pub struct BrewCask {
    pub token: String,
    pub full_token: String,
    pub tap: String,
    pub name: Vec<String>,
    pub desc: Option<String>,
    pub homepage: String,
    pub version: String,
    pub installed: Option<String>,
    pub outdated: bool,
}

#[derive(Debug, Deserialize)]
pub struct BrewVersions {
    pub stable: Option<String>,
    pub head: Option<String>,
    pub bottle: bool,
}

#[derive(Debug, Deserialize)]
pub struct BrewInstalled {
    pub version: String,
    pub used_options: Vec<String>,
    pub built_as_bottle: bool,
    pub poured_from_bottle: bool,
    pub time: u64,
    pub runtime_dependencies: Vec<BrewDependency>,
    pub installed_as_dependency: bool,
    pub installed_on_request: bool,
}

#[derive(Debug, Deserialize)]
pub struct BrewDependency {
    pub full_name: Option<String>,
    pub version: Option<String>,
    pub revision: Option<u32>,
    pub bottle_rebuild: Option<u32>,
    pub pkg_version: Option<String>,
    pub declared_directly: Option<bool>,
}

impl PackageInfo {
    /// Creates a new PackageInfo instance
    pub fn new(
        name: String,
        description: String,
        homepage: String,
        current_version: String,
        installed_version: Option<String>,
    ) -> Self {
        Self {
            name,
            description,
            homepage,
            current_version,
            installed_version,
            package_type: PackageType::Unknown,
            tap: None,
            outdated: false,
        }
    }

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
        }
    }

    /// Creates a new PackageInfo instance with full information including outdated status
    pub fn new_with_full_info(
        name: String,
        description: String,
        homepage: String,
        current_version: String,
        installed_version: Option<String>,
        package_type: PackageType,
        tap: Option<String>,
        outdated: bool,
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

    /// Gets the display name with package type prefix
    pub fn get_display_name(&self) -> String {
        match self.package_type {
            PackageType::Formulae => format!("⚙️ {}", self.name),
            PackageType::Cask => format!("🍺 {}", self.name),
            PackageType::Unknown => self.name.clone(),
        }
    }

    /// Gets an emoji representing the package type
    pub fn get_type_emoji(&self) -> &'static str {
        match self.package_type {
            PackageType::Formulae => "⚙️",
            PackageType::Cask => "🍺",
            PackageType::Unknown => "❓",
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
        };
        assert_eq!(package3.has_update_available(), true);
    }
}
