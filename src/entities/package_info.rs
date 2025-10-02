use crate::helpers;
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use nestify::nest;


nest! {
    #[derive(Debug, Clone)]
    pub struct PackageInfo {
        pub name: String,
        pub description: String,
        pub homepage: String,
        pub current_version: String,
        pub installed_version: Option<String>,
        pub package_type:
            #[derive(Debug, Clone, PartialEq)]
            pub enum PackageType {
                Formulae,
                Cask,
                Unknown,
            },
        pub tap: Option<String>,
        pub outdated: bool,
        pub caveats: Option<String>,
        pub installed_at: Option<u64>, // Unix timestamp
    }
}

impl PackageInfo {
    /// Creates a new PackageInfo instance with all information
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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

    /// Checks if the package has an update available
    pub fn has_update_available(&self) -> bool {
        match &self.installed_version {
            Some(installed) => {
                // Use version comparison that understands revisions
                match helpers::compare_homebrew_versions(installed, &self.current_version) {
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
                if self.has_update_available() {
                    format!("{} (update available)", version)
                } else {
                    format!("{} (up to date)", version)
                }
            }
            None => "Not installed".to_string(),
        }
    }

    /// Returns a human-readable string indicating how long ago the package was installed
    pub fn installed_ago(&self) -> Option<String> {
        if let Some(timestamp) = self.installed_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();

            if now >= timestamp {
                let diff = now - timestamp;
                return Some(helpers::format_time_ago(diff));
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
