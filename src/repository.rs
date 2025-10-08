use crate::entities::brew_info_response::BrewInfoResponse;
use crate::entities::package_info::{PackageInfo, PackageType};
use crate::helpers;
use anyhow::Result;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct HomebrewRepository {
    installed_packages: Arc<Mutex<Vec<PackageInfo>>>,
    cache: Arc<Mutex<HashMap<String, PackageInfo>>>,
    uninstalled_packages: Arc<Mutex<HashMap<String, Instant>>>, // Track recently uninstalled packages
}

impl HomebrewRepository {
    pub fn new() -> Self {
        let installed_packages =  Self::load_installed_packages();
        let cache = Arc::new(Mutex::new(HashMap::new()));

        Self {
            installed_packages: Arc::new(Mutex::new(installed_packages)),
            cache,
            uninstalled_packages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Load all installed packages from Homebrew
    fn load_installed_packages() -> Vec<PackageInfo> {
        match helpers::brew_info_all_installed() {
            Ok(brew_response) => {
                let mut packages = Self::process_brew_response(brew_response);

                // If no packages are found, show a helpful message
                if packages.is_empty() {
                    packages.push(Self::create_no_packages_placeholder());
                }
                packages
            }
            Err(err) => vec![Self::create_error_placeholder(err)],
        }
    }

    /// Process a BrewInfoResponse and return a list of directly installed packages
    fn process_brew_response(brew_response: BrewInfoResponse) -> Vec<PackageInfo> {
        let mut packages = Vec::new();

        // Process formulae - only include packages installed directly (not as dependencies)
        for formula in brew_response.formulae {
            let is_directly_installed = formula.installed.iter().any(|install_info| {
                install_info.installed_on_request || !install_info.installed_as_dependency
            });
            if !is_directly_installed {
                continue;
            }
            packages.push(PackageInfo::from(&formula));
        }

        // Process casks
        for cask in brew_response.casks {
            packages.push(PackageInfo::from(&cask));
        }

        packages
    }

    /// Create a placeholder for when no packages are installed
    fn create_no_packages_placeholder() -> PackageInfo {
        PackageInfo::new(
            "no-packages".to_string(),
            "No packages are currently installed via Homebrew. Use 'brew install <package>' to install packages.".to_string(),
            "https://brew.sh".to_string(),
            "1.0.0".to_string(),
            None,
            PackageType::Unknown,
            None,
            false,
            None,
            None,
        )
    }

    /// Create a placeholder for when there's an error loading packages
    fn create_error_placeholder(err: anyhow::Error) -> PackageInfo {
        PackageInfo::new(
            "homebrew-error".to_string(),
            format!(
                "Error loading packages from Homebrew: {}. Make sure Homebrew is installed and accessible.",
                err
            ),
            "https://brew.sh".to_string(),
            "1.0.0".to_string(),
            None,
            PackageType::Unknown,
            None,
            false,
            None,
            None,
        )
    }

    /// Get all installed packages
    pub fn get_all_packages(&self) -> Result<Vec<PackageInfo>> {
        let now = Instant::now();

        // Clean up old uninstalled packages (older than 30 seconds)
        if let Ok(mut uninstalled) = self.uninstalled_packages.lock() {
            uninstalled
                .retain(|_, timestamp| now.duration_since(*timestamp) < Duration::from_secs(30));
        }

        // Filter out recently uninstalled packages
        let blacklisted_packages: HashSet<String> =
            if let Ok(uninstalled) = self.uninstalled_packages.lock() {
                uninstalled.keys().cloned().collect()
            } else {
                HashSet::new()
            };

        let filtered_packages: Vec<PackageInfo> =
            if let Ok(installed_guard) = self.installed_packages.lock() {
                installed_guard
                    .iter()
                    .filter(|pkg| !blacklisted_packages.contains(&pkg.name))
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };

        Ok(filtered_packages)
    }

    /// Uninstall a package by name
    pub fn uninstall_package(&self, package_name: &str) -> Result<()> {
        let output = Command::new("brew")
            .args(["uninstall", package_name])
            .output()?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to uninstall {}: {}",
                package_name,
                error_msg
            ));
        }

        Ok(())
    }

    /// Update a package by name
    pub fn update_package(&self, package_name: &str) -> Result<()> {
        let output = Command::new("brew")
            .args(["upgrade", package_name])
            .output()?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to update {}: {}",
                package_name,
                error_msg
            ));
        }

        Ok(())
    }

    /// Refresh package details by name
    pub fn refresh_package(&self, package_name: &str) -> Result<Option<PackageInfo>> {
        // Get fresh information for a specific package
        let output = Command::new("brew")
            .args(["info", "--json=v2", package_name])
            .output()?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to get info for {}: {}",
                package_name,
                error_msg
            ));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let brew_response: BrewInfoResponse = serde_json::from_str(&json_str)?;

        // Process formulae
        for formula in brew_response.formulae {
            if formula.name == package_name {
                // Check if this formulae was installed directly
                let is_directly_installed = formula.installed.iter().any(|install_info| {
                    install_info.installed_on_request || !install_info.installed_as_dependency
                });

                if !is_directly_installed {
                    return Ok(None); // Not a direct installation
                }

                let latest_install = formula.installed.iter().max_by(|a, b| {
                    // First compare by timestamp
                    let time_cmp = a.time.cmp(&b.time);
                    if time_cmp != Ordering::Equal {
                        return time_cmp;
                    }
                    // If timestamps are equal, compare versions (considering _X revisions)
                    helpers::compare_homebrew_versions(&a.version, &b.version)
                });

                let (installed_version, installed_at) = match latest_install {
                    Some(install) => (Some(install.version.clone()), Some(install.time)),
                    None => (None, None),
                };

                let current_version = formula.versions.stable.unwrap_or_else(|| {
                    formula
                        .versions
                        .head
                        .unwrap_or_else(|| "unknown".to_string())
                });

                let package_info = PackageInfo::new(
                    formula.name.clone(),
                    formula.desc,
                    formula.homepage,
                    current_version,
                    installed_version,
                    PackageType::Formulae,
                    formula.tap,
                    formula.outdated,
                    formula.caveats,
                    installed_at,
                );

                return Ok(Some(package_info));
            }
        }

        // Process casks
        for cask in brew_response.casks {
            if cask.token == package_name {
                let display_name = cask.name.first().unwrap_or(&cask.token).clone();
                let description = cask
                    .desc
                    .unwrap_or_else(|| format!("{} (Cask application)", display_name));

                let package_info = PackageInfo::new(
                    cask.token,
                    description,
                    cask.homepage,
                    cask.version.clone(),
                    cask.installed.clone(),
                    PackageType::Cask,
                    cask.tap,
                    cask.outdated,
                    cask.caveats,
                    None, // Casks don't have installation timestamp in the JSON
                );

                return Ok(Some(package_info));
            }
        }

        Ok(None) // Package not found
    }

    /// Clear package cache and mark as uninstalled
    pub fn clear_package_cache(&self, package_name: &str) {
        let now = Instant::now();

        // Remove package from cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(package_name);
        }

        // Add to uninstalled blacklist to prevent re-adding for 30 seconds
        if let Ok(mut uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.insert(package_name.to_string(), now);
        }
    }

    /// Refresh all packages information from Homebrew
    pub fn refresh_all_packages(&self) -> Result<()> {
        // Reload all installed packages from brew
        let new_packages = Self::load_installed_packages();

        // Update the installed packages list
        if let Ok(mut installed_guard) = self.installed_packages.lock() {
            *installed_guard = new_packages;
        }

        // Clear the uninstalled packages blacklist since we have fresh data
        if let Ok(mut uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.clear();
        }

        Ok(())
    }
}
