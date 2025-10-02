use crate::helpers;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use std::process::Command;
use std::cmp::Ordering;
use crate::entities::brew_info_response::BrewInfoResponse;
use crate::entities::package_info::{PackageInfo, PackageType};

#[derive(Debug, Clone)]
struct PackageRequest {
    package_name: String,
    priority: bool, // true for currently selected package
    requested_at: Instant,
}

// Real Homebrew repository that fetches only installed packages
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
}

impl HomebrewRepository {
    pub fn new() -> Self {
        let mut installed_packages = Vec::new();
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
        match helpers::brew_info_all_installed() {
            Ok(brew_response) => {
                // Process formulae - only include packages installed directly (not as dependencies)
                for formula in brew_response.formulae {
                    let is_directly_installed = formula.installed.iter().any(|install_info| {
                        install_info.installed_on_request || !install_info.installed_as_dependency
                    });
                    if !is_directly_installed {
                        continue;
                    }
                    installed_packages.push(helpers::brew_formulae_to_package_info(&formula));
                }
                // Process casks
                for cask in brew_response.casks {
                    installed_packages.push(helpers::brew_cask_to_package_info(&cask));
                }
                
                // If no packages are found, show a helpful message
                if installed_packages.is_empty() {
                    installed_packages.push(PackageInfo::new(
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
                    ));
                }
            }
            Err(err) => {
                // If we can't load packages, provide a detailed error message
                installed_packages.push(PackageInfo::new(
                    "homebrew-error".to_string(),
                    format!("Error loading packages from Homebrew: {}. Make sure Homebrew is installed and accessible.", err),
                    "https://brew.sh".to_string(),
                    "1.0.0".to_string(),
                    None,
                    PackageType::Unknown,
                    None,
                    false,
                    None,
                    None,
                ));
            }
        }
        
        Self {
            installed_packages: Arc::new(Mutex::new(installed_packages)),
            cache,
            current_status,
            request_sender,
            current_request,
            pending_requests,
            loading_animation_state,
            last_animation_update,
            uninstalled_packages: Arc::new(Mutex::new(HashMap::new())),
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
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
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
                match helpers::brew_info(&request.package_name) {
                    Ok(package) => {
                        let mut to_cache = Vec::new();
                        
                        // Look for matching formulae
                        for formula in &package.formulae {
                            let pkg_info = helpers::brew_formulae_to_package_info(formula);
                            to_cache.push((formula.name.clone(), pkg_info));
                        }
                        
                        // Look for matching cask
                        for cask in &package.casks {
                            let pkg_info = helpers::brew_cask_to_package_info(cask);
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
    
    /// Get all installed packages
    pub fn get_all_packages(&self) -> Result<Vec<PackageInfo>> {
        let now = Instant::now();
        
        // Clean up old uninstalled packages (older than 30 seconds)
        if let Ok(mut uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.retain(|_, timestamp| {
                now.duration_since(*timestamp) < Duration::from_secs(30)
            });
        }
        
        // Filter out recently uninstalled packages
        let blacklisted_packages: HashSet<String> = if let Ok(uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.keys().cloned().collect()
        } else {
            HashSet::new()
        };
        
        let filtered_packages: Vec<PackageInfo> = if let Ok(installed_guard) = self.installed_packages.lock() {
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


    /// Get package details by name
    pub fn get_package_details(&self, package_name: &str) -> Option<PackageInfo> {
        // First check if we have it cached (detailed info)
        {
            let cache_guard = self.cache.lock().unwrap();
            if let Some(cached_package) = cache_guard.get(package_name) {
                return Some(cached_package.clone());
            }
        }
        
        // Look for the package in our installed packages list
        let placeholder = if let Ok(installed_guard) = self.installed_packages.lock() {
            installed_guard
                .iter()
                .find(|pkg| pkg.name == package_name)
                .cloned()
        } else {
            None
        };
            
        if let Some(pkg) = placeholder {
            return if pkg.description.starts_with("Loading package information") {
                // Request with HIGH PRIORITY for currently selected package
                self.request_package_details(package_name.to_string(), true);

                // Return an updated placeholder with animated loading text
                let animated_loading_text = self.get_animated_loading_text();
                let mut updated_pkg = pkg.clone();
                updated_pkg.description = animated_loading_text;
                Some(updated_pkg)
            } else {
                // Return the existing detailed info
                Some(pkg.clone())
            }
        }
        
        // If not found in installed packages, request with normal priority
        self.request_package_details(package_name.to_string(), false);
        None
    }

    /// Uninstall a package by name
    pub fn uninstall_package(&self, package_name: &str) -> Result<()> {
        let output = Command::new("brew")
            .args(["uninstall", package_name])
            .output()?;
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to uninstall {}: {}", package_name, error_msg));
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
            return Err(anyhow::anyhow!("Failed to update {}: {}", package_name, error_msg));
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
            return Err(anyhow::anyhow!("Failed to get info for {}: {}", package_name, error_msg));
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
                
                let latest_install = formula.installed
                    .iter()
                    .max_by(|a, b| {
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
                
                let current_version = formula.versions.stable
                    .unwrap_or_else(|| formula.versions.head.unwrap_or_else(|| "unknown".to_string()));
                
                let package_info = PackageInfo::new(
                    formula.name.clone(),
                    formula.desc,
                    formula.homepage,
                    current_version,
                    installed_version,
                    PackageType::Formulae,
                    Some(formula.tap),
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
                let display_name = cask.name.first()
                    .unwrap_or(&cask.token)
                    .clone();
                let description = cask.desc
                    .unwrap_or_else(|| format!("{} (Cask application)", display_name));
                
                let package_info = PackageInfo::new(
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

    /// Clear package cache and mark as uninstalled
    pub fn clear_package_cache(&self, package_name: &str) {
        let now = Instant::now();
        
        // Remove package from cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(package_name);
        }
        
        // Remove from pending requests
        if let Ok(mut pending) = self.pending_requests.lock() {
            pending.remove(package_name);
        }
        
        // Add to uninstalled blacklist to prevent re-adding for 30 seconds
        if let Ok(mut uninstalled) = self.uninstalled_packages.lock() {
            uninstalled.insert(package_name.to_string(), now);
        }
    }
    
    /// Refresh all packages information from Homebrew
    pub fn refresh_all_packages(&self) -> Result<()> {
        // Reload all installed packages from brew
        match helpers::brew_info_all_installed() {
            Ok(brew_response) => {
                let mut new_packages = Vec::new();
                
                // Process formulae - only include packages installed directly (not as dependencies)
                for formula in brew_response.formulae {
                    let is_directly_installed = formula.installed.iter().any(|install_info| {
                        install_info.installed_on_request || !install_info.installed_as_dependency
                    });
                    if !is_directly_installed {
                        continue;
                    }
                    new_packages.push(helpers::brew_formulae_to_package_info(&formula));
                }
                
                // Process casks
                for cask in brew_response.casks {
                    new_packages.push(helpers::brew_cask_to_package_info(&cask));
                }
                
                // If no packages are found, show a helpful message
                if new_packages.is_empty() {
                    new_packages.push(PackageInfo::new(
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
                    ));
                }
                
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
            Err(err) => {
                Err(anyhow::anyhow!("Failed to refresh package list: {}", err))
            }
        }
    }
}
