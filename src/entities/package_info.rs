use crate::entities::brew_info_response::{BrewCask, BrewFormula};
use crate::entities::mas_app::MasApp;
use crate::helpers;
use nestify::nest;
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

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
                MasApp,
                Unknown,
            },
        pub tap: Option<String>,
        pub outdated: bool,
        pub caveats: Option<String>,
        pub installed_at: Option<u64>, // Unix timestamp
        pub mas_id: Option<u32>,
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
        mas_id: Option<u32>,
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
            mas_id,
        }
    }

    /// Checks if the package has an update available
    pub fn has_update_available(&self) -> bool {
        // Trust Homebrew's outdated flag first - it knows about revisions and other subtleties
        if self.outdated {
            return true;
        }

        // Fallback to version comparison for cases where outdated flag might not be set
        match &self.installed_version {
            Some(installed) => {
                // Use version comparison that understands revisions
                match helpers::compare_homebrew_versions(installed, &self.current_version) {
                    Ordering::Less => true, // installed < current, update available
                    Ordering::Equal | Ordering::Greater => false, // installed >= current, no update needed
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
                    // Check if this is a revision update (versions appear the same but outdated flag is set)
                    if self.outdated && version == &self.current_version {
                        format!("{} (revision update available)", version)
                    } else {
                        format!("{} (update available)", version)
                    }
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
            let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

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
            PackageType::MasApp => format!("🍏 {}", self.name),
            PackageType::Unknown => self.name.clone(),
        }
    }
}

impl From<&BrewFormula> for PackageInfo {
    fn from(formula: &BrewFormula) -> Self {
        let (installed_version, installed_at) = if !formula.installed.is_empty() {
            let latest_install = formula
                .installed
                .iter()
                .max_by_key(|install| install.time.unwrap_or(0));
            match latest_install {
                Some(install) => (Some(install.version.clone()), install.time),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        PackageInfo::new(
            formula.name.clone(),
            formula.desc.clone(),
            formula
                .homepage
                .clone()
                .unwrap_or_else(|| "No homepage available".to_string()),
            formula
                .versions
                .stable
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            installed_version,
            PackageType::Formulae,
            formula.tap.clone(),
            formula.outdated,
            formula.caveats.clone(),
            installed_at,
            None,
        )
    }
}

impl From<&BrewCask> for PackageInfo {
    fn from(cask: &BrewCask) -> Self {
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
            cask.homepage
                .clone()
                .unwrap_or_else(|| "No homepage available".to_string()),
            cask.version.clone(),
            installed_version,
            PackageType::Cask,
            cask.tap.clone().map(|tap| format!("{} (cask)", tap)),
            cask.outdated,
            cask.caveats.clone(),
            None, // Casks don't have installation timestamp in the JSON
            None,
        )
    }
}

impl From<&MasApp> for PackageInfo {
    fn from(app: &MasApp) -> Self {
        let homepage = format!("https://apps.apple.com/app/id{}", app.id);
        let current_version = app
            .available_version
            .clone()
            .unwrap_or_else(|| app.version.clone());

        PackageInfo::new(
            app.name.clone(),
            String::new(),
            homepage,
            current_version,
            Some(app.version.clone()),
            PackageType::MasApp,
            Some("mac-app-store".to_string()),
            app.outdated,
            None,
            None,
            Some(app.id),
        )
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
            installed_at: Some(1696118400),
            mas_id: None,
        };
        assert!(!package1.has_update_available());

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
            installed_at: Some(1696118400),
            mas_id: None,
        };
        assert!(package2.has_update_available());

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
            installed_at: Some(1696118400),
            mas_id: None,
        };
        assert!(package3.has_update_available());
    }

    #[test]
    fn test_from_brew_formula_with_null_homepage() {
        use crate::entities::brew_info_response::{BrewFormula, BrewVersions};

        let formula = BrewFormula {
            name: "test-formula".to_string(),
            tap: Some("homebrew/core".to_string()),
            desc: "Test description".to_string(),
            homepage: None,
            versions: BrewVersions {
                stable: Some("1.0.0".to_string()),
                head: None,
            },
            installed: vec![],
            outdated: false,
            caveats: None,
        };

        let package_info = PackageInfo::from(&formula);
        assert_eq!(package_info.name, "test-formula");
        assert_eq!(package_info.homepage, "No homepage available");
        assert_eq!(package_info.description, "Test description");
    }

    #[test]
    fn test_from_brew_cask_with_null_homepage() {
        use crate::entities::brew_info_response::BrewCask;

        let cask = BrewCask {
            token: "test-cask".to_string(),
            tap: Some("homebrew/cask".to_string()),
            name: vec!["Test Cask".to_string()],
            desc: Some("Test description".to_string()),
            homepage: None,
            version: "1.0.0".to_string(),
            installed: None,
            outdated: false,
            caveats: None,
        };

        let package_info = PackageInfo::from(&cask);
        assert_eq!(package_info.name, "test-cask");
        assert_eq!(package_info.homepage, "No homepage available");
        assert_eq!(package_info.description, "Test description");
    }

    #[test]
    fn test_from_mas_app_uses_available_version_as_current() {
        let mas_app = MasApp {
            id: 310633997,
            name: "WhatsApp".to_string(),
            version: "26.18.72".to_string(),
            available_version: Some("26.19.75".to_string()),
            outdated: true,
        };

        let package_info = PackageInfo::from(&mas_app);
        assert_eq!(package_info.current_version, "26.19.75");
        assert_eq!(package_info.installed_version.as_deref(), Some("26.18.72"));
        assert!(package_info.has_update_available());
    }
}
