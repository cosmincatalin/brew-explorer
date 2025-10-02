use crate::models::{PackageInfo, PackageType, BrewInfoResponse, BrewFormulae, BrewCask};
use anyhow::Result;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use std::process::Command;
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

/// Trait for package repository operations
pub trait PackageRepository: Any + Send + Sync {
    fn get_all_packages(&self) -> Result<Vec<PackageInfo>>;
    fn get_package_details(&self, package_name: &str) -> Option<PackageInfo>;
    fn uninstall_package(&self, package_name: &str) -> Result<()>;
    fn update_package(&self, package_name: &str) -> Result<()>;
    fn refresh_package(&self, package_name: &str) -> Result<Option<PackageInfo>>;
    fn clear_package_cache(&self, package_name: &str);
    
    /// Allows downcasting to concrete types
    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Clone)]
struct PackageRequest {
    package_name: String,
    priority: bool, // true for currently selected package
    requested_at: Instant,
}

/// Real Homebrew repository that fetches only installed packages
pub struct HomebrewRepository {
    installed_packages: Arc<Mutex<Vec<PackageInfo>>>,
    cache: Arc<Mutex<HashMap<String, PackageInfo>>>,
    current_status: Arc<Mutex<Option<String>>>,
    request_sender: Sender<PackageRequest>,
    current_request: Arc<Mutex<Option<String>>>, // Currently processing package
    pending_requests: Arc<Mutex<HashMap<String, Instant>>>, // Track pending requests
    loading_animation_state: Arc<Mutex<usize>>, // For animated loading dots
    last_animation_update: Arc<Mutex<Instant>>, // Track last animation update
    uninstalled_packages: Arc<Mutex<HashMap<String, Instant>>>, // Track recently uninstalled packages
    last_package_list_update: Arc<Mutex<Instant>>, // Track when package list was last refreshed
    force_refresh_on_next_call: Arc<Mutex<bool>>, // Flag to force refresh
}

/// Helper functions for calling brew commands
fn brew_info_all_installed() -> Result<BrewInfoResponse> {
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

fn brew_info(package_name: &str) -> Result<BrewInfoResponse> {
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

impl HomebrewRepository {
    pub fn new() -> Self {
        let mut initial_packages = Vec::new();
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let current_status = Arc::new(Mutex::new(None));
        let current_request = Arc::new(Mutex::new(None));
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let loading_animation_state = Arc::new(Mutex::new(0));
        let last_animation_update = Arc::new(Mutex::new(Instant::now()));
        
        // Create request channel
        let (request_sender, request_receiver) = mpsc::channel::<PackageRequest>();
        
        // Start request processor thread
        let cache_clone = cache.clone();
        let status_clone = current_status.clone();
        let current_request_clone = current_request.clone();
        let pending_requests_clone = pending_requests.clone();
        
        thread::spawn(move || {
            Self::process_requests(request_receiver, cache_clone, status_clone, current_request_clone, pending_requests_clone);
        });
        
        // Load all installed packages with full information immediately (synchronously)
        match brew_info_all_installed() {
            Ok(brew_response) => {
                // Process formulae - only include packages installed directly (not as dependencies)
                for formulae in brew_response.formulae {
                    // Check if this formulae was installed directly by looking at the installation info
                    let is_directly_installed = formulae.installed.iter().any(|install_info| {
                        install_info.installed_on_request || !install_info.installed_as_dependency
                    });
                    
                    // Skip packages that were only installed as dependencies
                    if !is_directly_installed {
                        continue;
                    }
                    
                    let latest_install = formulae.installed
                        .iter()
                        .max_by(|a, b| {
                            // First compare by timestamp
                            let time_cmp = a.time.cmp(&b.time);
                            if time_cmp != std::cmp::Ordering::Equal {
                                return time_cmp;
                            }
                            // If timestamps are equal, compare versions (considering _X revisions)
                            compare_homebrew_versions(&a.version, &b.version)
                        });
                    
                    let (installed_version, installed_at) = match latest_install {
                        Some(install) => (Some(install.version.clone()), Some(install.time)),
                        None => (None, None),
                    };
                    
                    let current_version = formulae.versions.stable
                        .unwrap_or_else(|| formulae.versions.head.unwrap_or_else(|| "unknown".to_string()));
                    
                    initial_packages.push(PackageInfo::new_with_caveats(
                        formulae.name.clone(),
                        formulae.desc,
                        formulae.homepage,
                        current_version,
                        installed_version,
                        PackageType::Formulae,
                        Some(formulae.tap),
                        formulae.outdated,
                        formulae.caveats,
                        installed_at,
                    ));
                }
                
                // Process casks
                for cask in brew_response.casks {
                    let display_name = cask.name.first()
                        .unwrap_or(&cask.token)
                        .clone();
                    let description = cask.desc
                        .unwrap_or_else(|| format!("{} (Cask application)", display_name));
                    
                    initial_packages.push(PackageInfo::new_with_caveats(
                        cask.token,
                        description,
                        cask.homepage,
                        cask.version.clone(),
                        cask.installed.clone(),
                        PackageType::Cask,
                        Some(cask.tap),
                        cask.outdated,
                        cask.caveats,
                        None, // Casks don't have installation timestamp in the JSON
                    ));
                }
                
                // If no packages are found, show a helpful message
                if initial_packages.is_empty() {
                    initial_packages.push(PackageInfo::new_with_type(
                        "no-packages".to_string(),
                        "No packages are currently installed via Homebrew. Use 'brew install <package>' to install packages.".to_string(),
                        "https://brew.sh".to_string(),
                        "1.0.0".to_string(),
                        None,
                        PackageType::Unknown,
                        None,
                    ));
                }
            }
            Err(err) => {
                // If we can't load packages, provide a detailed error message
                initial_packages.push(PackageInfo::new_with_type(
                    "homebrew-error".to_string(),
                    format!("Error loading packages from Homebrew: {}. Make sure Homebrew is installed and accessible.", err),
                    "https://brew.sh".to_string(),
                    "1.0.0".to_string(),
                    None,
                    PackageType::Unknown,
                    None,
                ));
            }
        }
        
        Self {
            installed_packages: Arc::new(Mutex::new(initial_packages)),
            cache,
            current_status,
            request_sender,
            current_request,
            pending_requests,
            loading_animation_state,
            last_animation_update,
            uninstalled_packages: Arc::new(Mutex::new(HashMap::new())),
            last_package_list_update: Arc::new(Mutex::new(Instant::now())),
            force_refresh_on_next_call: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Process package requests with priority and cancellation
    fn process_requests(
        receiver: Receiver<PackageRequest>,
        cache: Arc<Mutex<HashMap<String, PackageInfo>>>,
        status: Arc<Mutex<Option<String>>>,
        current_request: Arc<Mutex<Option<String>>>,
        pending_requests: Arc<Mutex<HashMap<String, Instant>>>,
    ) {
        let mut pending_queue: Vec<PackageRequest> = Vec::new();
        
        loop {
            // Try to receive new requests with a timeout
            let timeout_duration = Duration::from_millis(100);
            
            // Collect all pending requests
            while let Ok(request) = receiver.recv_timeout(timeout_duration) {
                // Remove old request for the same package if exists
                pending_queue.retain(|r| r.package_name != request.package_name);
                pending_queue.push(request);
            }
            
            if pending_queue.is_empty() {
                continue;
            }
            
            // Sort by priority (priority requests first, then by request time)
            pending_queue.sort_by(|a, b| {
                match (a.priority, b.priority) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.requested_at.cmp(&b.requested_at),
                }
            });
            
            // Process the highest priority request
            if let Some(request) = pending_queue.pop() {
                // Check if already cached
                {
                    let cache_guard = cache.lock().unwrap();
                    if cache_guard.contains_key(&request.package_name) {
                        continue; // Skip if already cached
                    }
                }
                
                // Mark as current request
                {
                    let mut current_guard = current_request.lock().unwrap();
                    *current_guard = Some(request.package_name.clone());
                }
                
                // Update status
                {
                    let mut status_guard = status.lock().unwrap();
                    *status_guard = Some(format!("Loading details for {}", request.package_name));
                }
                
                // Mark as pending
                {
                    let mut pending_guard = pending_requests.lock().unwrap();
                    pending_guard.insert(request.package_name.clone(), Instant::now());
                }
                
                // Fetch package info
                match brew_info(&request.package_name) {
                    Ok(package) => {
                        let mut to_cache = Vec::new();
                        
                        // Look for matching formulae
                        for formulae in &package.formulae {
                            let pkg_info = brew_formulae_to_package_info(formulae);
                            to_cache.push((formulae.name.clone(), pkg_info));
                        }
                        
                        // Look for matching cask
                        for cask in &package.casks {
                            let pkg_info = brew_cask_to_package_info(cask);
                            to_cache.push((cask.token.clone(), pkg_info));
                        }
                        
                        // Cache the results
                        {
                            let mut cache_guard = cache.lock().unwrap();
                            for (name, pkg_info) in to_cache {
                                cache_guard.insert(name, pkg_info);
                            }
                        }
                    }
                    Err(_) => {
                        // Ignore errors for individual packages
                    }
                }
                
                // Clear current request and pending status
                {
                    let mut current_guard = current_request.lock().unwrap();
                    *current_guard = None;
                }
                {
                    let mut pending_guard = pending_requests.lock().unwrap();
                    pending_guard.remove(&request.package_name);
                }
                {
                    let mut status_guard = status.lock().unwrap();
                    *status_guard = None;
                }
            }
        }
    }
    
    /// Get the current status message from background operations
    pub fn get_current_status(&self) -> Option<String> {
        let status_guard = self.current_status.lock().unwrap();
        status_guard.clone()
    }
    
    /// Get animated loading text with cycling dots
    pub fn get_animated_loading_text(&self) -> String {
        // Update animation state every 400ms for faster animation
        let should_update = {
            let last_update = self.last_animation_update.lock().unwrap();
            last_update.elapsed() >= Duration::from_millis(400)
        };
        
        if should_update {
            {
                let mut last_update = self.last_animation_update.lock().unwrap();
                *last_update = Instant::now();
            }
            {
                let mut state = self.loading_animation_state.lock().unwrap();
                *state = (*state + 1) % 3; // Cycle through 0, 1, 2
            }
        }
        
        let state = {
            let state_guard = self.loading_animation_state.lock().unwrap();
            *state_guard
        };
        
        match state {
            0 => "Loading package information.".to_string(),
            1 => "Loading package information..".to_string(),
            2 => "Loading package information...".to_string(),
            _ => "Loading package information...".to_string(),
        }
    }
    
    /// Update loading animations for packages with loading placeholders
    pub fn update_loading_animations(&self) -> bool {
        // This method will be called to check if animations need updating
        // We return true if any updates occurred to signal the UI to refresh
        
        
        {
            let last_update = self.last_animation_update.lock().unwrap();
            last_update.elapsed() >= Duration::from_millis(400)
        }
    }
    pub fn has_updated_details(&self, package_name: &str) -> bool {
        let cache_guard = self.cache.lock().unwrap();
        cache_guard.contains_key(package_name)
    }
    
    /// Get updated package details from cache (non-blocking)
    pub fn get_cached_details(&self, package_name: &str) -> Option<PackageInfo> {
        let cache_guard = self.cache.lock().unwrap();
        cache_guard.get(package_name).cloned()
    }
    
    /// Start background loading of package details (non-blocking)
    /// Request package details with priority
    pub fn request_package_details(&self, package_name: String, priority: bool) {
        // Check if this request is already being processed or is recent
        {
            let current = self.current_request.lock().unwrap();
            if let Some(ref current_name) = *current
                && *current_name == package_name {
                    return; // Already processing this package
                }
            
            let mut pending = self.pending_requests.lock().unwrap();
            if let Some(&timestamp) = pending.get(&package_name) {
                // If request was made less than 1 second ago, don't duplicate
                if timestamp.elapsed().as_secs() < 1 {
                    return;
                }
            }
            pending.insert(package_name.clone(), Instant::now());
        }
        
        let request = PackageRequest {
            package_name: package_name.clone(),
            priority,
            requested_at: Instant::now(),
        };
        
        // Send the request to the background processor
        if self.request_sender.send(request).is_err() {
            // Channel is closed, ignore
        }
        
        // If this is a priority request, update current_request
        if priority {
            let mut current = self.current_request.lock().unwrap();
            *current = Some(package_name);
        }
    }
    
}

impl PackageRepository for HomebrewRepository {
    fn get_all_packages(&self) -> Result<Vec<PackageInfo>> {
        let now = Instant::now();
        
        // Clean up old uninstalled packages (older than 2 minutes)
        if let Ok(mut uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.retain(|_, timestamp| {
                now.duration_since(*timestamp) < Duration::from_secs(120)
            });
        }
        
        // Check if we need to force refresh or if enough time has passed (30 seconds)
        let should_refresh = {
            let force_refresh = if let Ok(mut force) = self.force_refresh_on_next_call.lock() {
                let should_force = *force;
                *force = false; // Reset the flag
                should_force
            } else {
                false
            };
            
            let time_based_refresh = if let Ok(last_update) = self.last_package_list_update.lock() {
                now.duration_since(*last_update) > Duration::from_secs(30)
            } else {
                true
            };
            
            force_refresh || time_based_refresh
        };
        
        // Get blacklisted packages
        let blacklisted_packages: HashSet<String> = if let Ok(uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.keys().cloned().collect()
        } else {
            HashSet::new()
        };
        
        if should_refresh {
            // Fetch fresh package list from Homebrew only when needed
            match brew_info_all_installed() {
                Ok(brew_response) => {
                    let mut fresh_packages = Vec::new();
                    
                    // Process formulae - only include packages installed directly (not as dependencies)
                    for formulae in brew_response.formulae {
                        // Skip blacklisted packages
                        if blacklisted_packages.contains(&formulae.name) {
                            continue;
                        }
                        
                        // Check if this formulae was installed directly by looking at the installation info
                        let is_directly_installed = formulae.installed.iter().any(|install_info| {
                            install_info.installed_on_request || !install_info.installed_as_dependency
                        });
                        
                        // Skip packages that were only installed as dependencies
                        if !is_directly_installed {
                            continue;
                        }
                        
                        let latest_install = formulae.installed
                            .iter()
                            .max_by(|a, b| {
                                // First compare by timestamp
                                let time_cmp = a.time.cmp(&b.time);
                                if time_cmp != std::cmp::Ordering::Equal {
                                    return time_cmp;
                                }
                                // If timestamps are equal, compare versions (considering _X revisions)
                                compare_homebrew_versions(&a.version, &b.version)
                            });
                        
                        let (installed_version, installed_at) = match latest_install {
                            Some(install) => (Some(install.version.clone()), Some(install.time)),
                            None => (None, None),
                        };
                        
                        let current_version = formulae.versions.stable
                            .unwrap_or_else(|| formulae.versions.head.unwrap_or_else(|| "unknown".to_string()));
                        
                        fresh_packages.push(PackageInfo::new_with_caveats(
                            formulae.name.clone(),
                            formulae.desc,
                            formulae.homepage,
                            current_version,
                            installed_version,
                            PackageType::Formulae,
                            Some(formulae.tap),
                            formulae.outdated,
                            formulae.caveats,
                            installed_at,
                        ));
                    }
                    
                    // Process casks
                    for cask in brew_response.casks {
                        // Skip blacklisted packages
                        if blacklisted_packages.contains(&cask.token) {
                            continue;
                        }
                        
                        let display_name = cask.name.first()
                            .unwrap_or(&cask.token)
                            .clone();
                        let description = cask.desc
                            .unwrap_or_else(|| format!("{} (Cask application)", display_name));
                        
                        fresh_packages.push(PackageInfo::new_with_caveats(
                            cask.token,
                            description,
                            cask.homepage,
                            cask.version.clone(),
                            cask.installed.clone(),
                            PackageType::Cask,
                            Some(cask.tap),
                            cask.outdated,
                            cask.caveats,
                            None, // Casks don't have installation timestamp in the JSON
                        ));
                    }
                    
                    // Update the cached package list and timestamp
                    if let Ok(mut last_update) = self.last_package_list_update.lock() {
                        *last_update = now;
                    }
                    
                    // Update the internal cached package list
                    if let Ok(mut installed_packages) = self.installed_packages.lock() {
                        *installed_packages = fresh_packages.clone();
                    }
                    
                    Ok(fresh_packages)
                }
                Err(_) => {
                    // Fallback to cached list if fresh fetch fails, but still filter out blacklisted packages
                    let filtered_packages: Vec<PackageInfo> = if let Ok(installed_packages) = self.installed_packages.lock() {
                        installed_packages
                            .iter()
                            .filter(|pkg| !blacklisted_packages.contains(&pkg.name))
                            .cloned()
                            .collect()
                    } else {
                        Vec::new()
                    };
                    
                    Ok(filtered_packages)
                }
            }
        } else {
            // Use cached data with blacklist filtering
            let filtered_packages: Vec<PackageInfo> = if let Ok(installed_packages) = self.installed_packages.lock() {
                installed_packages
                    .iter()
                    .filter(|pkg| !blacklisted_packages.contains(&pkg.name))
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            
            Ok(filtered_packages)
        }
    }


    fn get_package_details(&self, package_name: &str) -> Option<PackageInfo> {
        // First check if we have it cached (detailed info)
        {
            let cache_guard = self.cache.lock().unwrap();
            if let Some(cached_package) = cache_guard.get(package_name) {
                return Some(cached_package.clone());
            }
        }
        
        // Look for the package in our installed packages list
        let placeholder = if let Ok(installed_packages) = self.installed_packages.lock() {
            installed_packages
                .iter()
                .find(|pkg| pkg.name == package_name)
                .cloned()
        } else {
            None
        };
            
        if let Some(pkg) = placeholder {
            if pkg.description.starts_with("Loading package information") {
                // Request with HIGH PRIORITY for currently selected package
                self.request_package_details(package_name.to_string(), true);
                
                // Return an updated placeholder with animated loading text
                let animated_loading_text = self.get_animated_loading_text();
                let mut updated_pkg = pkg.clone();
                updated_pkg.description = animated_loading_text;
                return Some(updated_pkg);
            } else {
                // Return the existing detailed info
                return Some(pkg.clone());
            }
        }
        
        // If not found in installed packages, request with normal priority
        self.request_package_details(package_name.to_string(), false);
        None
    }

    fn uninstall_package(&self, package_name: &str) -> Result<()> {
        // For packages that might require password input, we need to run the command interactively
        // This will temporarily leave the TUI and allow proper password input
        use std::process::Stdio;
        
        let status = Command::new("brew")
            .args(["uninstall", package_name])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        
        if !status.success() {
            return Err(anyhow::anyhow!("Failed to uninstall {} (exit code: {})", package_name, status.code().unwrap_or(-1)));
        }
        
        Ok(())
    }

    fn update_package(&self, package_name: &str) -> Result<()> {
        let output = Command::new("brew")
            .args(["upgrade", package_name])
            .output()?;
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to update {}: {}", package_name, error_msg));
        }
        
        Ok(())
    }

    fn refresh_package(&self, package_name: &str) -> Result<Option<PackageInfo>> {
        // Get fresh information for a specific package
        let output = Command::new("brew")
            .args(["info", "--json=v2", package_name])
            .output()?;
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get info for {}: {}", package_name, error_msg));
        }
        
        let json_str = String::from_utf8_lossy(&output.stdout);
        let brew_response: BrewInfoResponse = serde_json::from_str(&json_str)?;
        
        // Process formulae
        for formulae in brew_response.formulae {
            if formulae.name == package_name {
                // Check if this formulae was installed directly
                let is_directly_installed = formulae.installed.iter().any(|install_info| {
                    install_info.installed_on_request || !install_info.installed_as_dependency
                });
                
                if !is_directly_installed {
                    return Ok(None); // Not a direct installation
                }
                
                let latest_install = formulae.installed
                    .iter()
                    .max_by(|a, b| {
                        // First compare by timestamp
                        let time_cmp = a.time.cmp(&b.time);
                        if time_cmp != std::cmp::Ordering::Equal {
                            return time_cmp;
                        }
                        // If timestamps are equal, compare versions (considering _X revisions)
                        compare_homebrew_versions(&a.version, &b.version)
                    });
                
                let (installed_version, installed_at) = match latest_install {
                    Some(install) => (Some(install.version.clone()), Some(install.time)),
                    None => (None, None),
                };
                
                let current_version = formulae.versions.stable
                    .unwrap_or_else(|| formulae.versions.head.unwrap_or_else(|| "unknown".to_string()));
                
                let package_info = PackageInfo::new_with_caveats(
                    formulae.name.clone(),
                    formulae.desc,
                    formulae.homepage,
                    current_version,
                    installed_version,
                    PackageType::Formulae,
                    Some(formulae.tap),
                    formulae.outdated,
                    formulae.caveats,
                    installed_at,
                );
                
                return Ok(Some(package_info));
            }
        }
        
        // Process casks
        for cask in brew_response.casks {
            if cask.token == package_name {
                let display_name = cask.name.first()
                    .unwrap_or(&cask.token)
                    .clone();
                let description = cask.desc
                    .unwrap_or_else(|| format!("{} (Cask application)", display_name));
                
                let package_info = PackageInfo::new_with_caveats(
                    cask.token,
                    description,
                    cask.homepage,
                    cask.version.clone(),
                    cask.installed.clone(),
                    PackageType::Cask,
                    Some(cask.tap),
                    cask.outdated,
                    cask.caveats,
                    None, // Casks don't have installation timestamp in the JSON
                );
                
                return Ok(Some(package_info));
            }
        }
        
        Ok(None) // Package not found
    }

    fn clear_package_cache(&self, package_name: &str) {
        let now = Instant::now();
        
        // Remove package from cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(package_name);
        }
        
        // Remove from pending requests
        if let Ok(mut pending) = self.pending_requests.lock() {
            pending.remove(package_name);
        }
        
        // Clear current request if it matches
        if let Ok(mut current) = self.current_request.lock()
            && let Some(ref current_name) = *current
                && current_name == package_name {
                    *current = None;
                }
        
        // Add to uninstalled blacklist to prevent re-adding for 2 minutes
        if let Ok(mut uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.insert(package_name.to_string(), now);
        }
        
        // Force a refresh on the next call to get_all_packages()
        if let Ok(mut force_refresh) = self.force_refresh_on_next_call.lock() {
            *force_refresh = true;
        }
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl HomebrewRepository {
    /// Force the next call to get_all_packages() to fetch fresh data
    pub fn force_refresh(&self) {
        if let Ok(mut force_refresh) = self.force_refresh_on_next_call.lock() {
            *force_refresh = true;
        }
    }
}

/// Convert a brew Formulae JSON to our PackageInfo structure
fn brew_formulae_to_package_info(formulae: &BrewFormulae) -> PackageInfo {
    let (installed_version, installed_at) = if !formulae.installed.is_empty() {
        let latest_install = formulae.installed
            .iter()
            .max_by_key(|install| install.time);
        match latest_install {
            Some(install) => (Some(install.version.clone()), Some(install.time)),
            None => (None, None),
        }
    } else {
        (None, None)
    };

    PackageInfo::new_with_caveats(
        formulae.name.clone(),
        formulae.desc.clone(),
        formulae.homepage.clone(),
        formulae.versions.stable.clone().unwrap_or_else(|| "unknown".to_string()),
        installed_version,
        PackageType::Formulae,
        Some(formulae.tap.clone()),
        formulae.outdated,
        formulae.caveats.clone(),
        installed_at,
    )
}

/// Convert a brew Cask JSON to our PackageInfo structure  
fn brew_cask_to_package_info(cask: &BrewCask) -> PackageInfo {
    let installed_version = cask.installed.clone();
    
    let description = cask.desc.clone().unwrap_or_else(|| {
        if cask.name.is_empty() {
            "No description available".to_string()
        } else {
            cask.name.join(", ")
        }
    });

    PackageInfo::new_with_caveats(
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
