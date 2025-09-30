/// Represents a Homebrew package with its metadata
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub description: String,
    pub homepage: String,
    pub current_version: String,
    pub installed_version: Option<String>,
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
        }
    }

    /// Checks if the package is installed
    pub fn is_installed(&self) -> bool {
        self.installed_version.is_some()
    }

    /// Checks if the package has an update available
    pub fn has_update_available(&self) -> bool {
        match &self.installed_version {
            Some(installed) => installed != &self.current_version,
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
}
