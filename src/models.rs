use serde::Deserialize;

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
}

/// Represents the type of a Homebrew package
#[derive(Debug, Clone, PartialEq)]
pub enum PackageType {
    Formula,
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
    pub full_name: String,
    pub version: String,
    pub revision: u32,
    pub pkg_version: String,
    pub declared_directly: bool,
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

    /// Gets the display name with appropriate emoji based on package type
    pub fn get_display_name(&self) -> String {
        match self.package_type {
            PackageType::Formula => format!("📦 {}", self.name),
            PackageType::Cask => format!("🖥️ {}", self.name),
            PackageType::Unknown => self.name.clone(),
        }
    }

    /// Gets the package type emoji
    pub fn get_type_emoji(&self) -> &'static str {
        match self.package_type {
            PackageType::Formula => "📦",
            PackageType::Cask => "🖥️",
            PackageType::Unknown => "❓",
        }
    }
}
