use crate::models::PackageInfo;
use anyhow::Result;
use homebrew::{info, list, Cask, Formula};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

/// Trait for package repository operations
pub trait PackageRepository: Any {
    fn get_all_packages(&self) -> Result<Vec<PackageInfo>>;
    fn get_installed_packages(&self) -> Result<Vec<PackageInfo>>;
    fn search_packages(&self, query: &str) -> Result<Vec<PackageInfo>>;
    fn get_package_details(&self, package_name: &str) -> Option<PackageInfo>;
    fn install_package(&self, package_name: &str) -> Result<()>;
    fn uninstall_package(&self, package_name: &str) -> Result<()>;
    fn update_package(&self, package_name: &str) -> Result<()>;
    
    /// Allows downcasting to concrete types
    fn as_any(&self) -> &dyn Any;
}

/// Mock implementation for testing and development
pub struct MockPackageRepository {
    packages: Vec<PackageInfo>,
}

#[derive(Debug, Clone)]
struct PackageRequest {
    package_name: String,
    priority: bool, // true for currently selected package
    requested_at: Instant,
}

/// Real Homebrew repository that fetches only installed packages
pub struct HomebrewRepository {
    installed_packages: Vec<PackageInfo>,
    cache: Arc<Mutex<HashMap<String, PackageInfo>>>,
    current_status: Arc<Mutex<Option<String>>>,
    request_sender: Sender<PackageRequest>,
    current_request: Arc<Mutex<Option<String>>>, // Currently processing package
    pending_requests: Arc<Mutex<HashMap<String, Instant>>>, // Track pending requests
}

impl HomebrewRepository {
    pub fn new() -> Self {
        let mut installed_packages = Vec::new();
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let current_status = Arc::new(Mutex::new(None));
        let current_request = Arc::new(Mutex::new(None));
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        
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
        
        // Load installed package names immediately (synchronously)
        match list() {
            Ok(package_names) => {
                // Create placeholder PackageInfo entries for each installed package
                // We'll fetch detailed info only when the package is selected
                for package_name in package_names {
                    installed_packages.push(PackageInfo::new(
                        package_name.clone(),
                        "Loading package information...".to_string(),
                        "".to_string(),
                        "...".to_string(),
                        Some("installed".to_string()), // Mark as installed since it came from list()
                    ));
                }
                
                // If no packages are found, show a helpful message
                if installed_packages.is_empty() {
                    installed_packages.push(PackageInfo::new(
                        "no-packages".to_string(),
                        "No packages are currently installed via Homebrew. Use 'brew install <package>' to install packages.".to_string(),
                        "https://brew.sh".to_string(),
                        "1.0.0".to_string(),
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
                ));
            }
        }
        
        Self {
            installed_packages,
            cache,
            current_status,
            request_sender,
            current_request,
            pending_requests,
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
                match info(&request.package_name) {
                    Ok(package) => {
                        let mut to_cache = Vec::new();
                        
                        // Look for matching formula
                        for formula in package.formulae() {
                            let pkg_info = formula_to_package_info(formula);
                            to_cache.push((formula.name.clone(), pkg_info));
                        }
                        
                        // Look for matching cask
                        for cask in package.casks() {
                            let pkg_info = cask_to_package_info(cask);
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
    
    /// Check if a package has updated details available in cache
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
    pub fn start_package_details_loading(&self, package_name: String) {
        self.request_package_details(package_name, false);
    }
    
    /// Request package details with priority
    pub fn request_package_details(&self, package_name: String, priority: bool) {
        let request = PackageRequest {
            package_name,
            priority,
            requested_at: Instant::now(),
        };
        
        // Send request (ignore errors if receiver is dropped)
        let _ = self.request_sender.send(request);
    }
    
    /// Get package info, fetching on-demand if not cached
    fn get_package_info(&self, package_name: &str) -> Option<PackageInfo> {
        // First check cache
        {
            let cache_guard = self.cache.lock().unwrap();
            if let Some(cached_info) = cache_guard.get(package_name) {
                return Some(cached_info.clone());
            }
        }
        
        // Not in cache, fetch on-demand
        match info(package_name) {
            Ok(package) => {
                let mut result = None;
                let mut to_cache = Vec::new();
                
                // Look for matching formula
                for formula in package.formulae() {
                    let pkg_info = formula_to_package_info(formula);
                    to_cache.push((formula.name.clone(), pkg_info.clone()));
                    
                    if formula.name == package_name {
                        result = Some(pkg_info);
                    }
                }
                
                // Look for matching cask
                for cask in package.casks() {
                    let pkg_info = cask_to_package_info(cask);
                    to_cache.push((cask.token.clone(), pkg_info.clone()));
                    
                    if cask.token == package_name {
                        result = Some(pkg_info);
                    }
                }
                
                // Cache the results
                {
                    let mut cache_guard = self.cache.lock().unwrap();
                    for (name, pkg_info) in to_cache {
                        cache_guard.insert(name, pkg_info);
                    }
                }
                
                result
            }
            Err(_err) => {
                // Don't print errors, just return None
                None
            }
        }
    }
}

impl PackageRepository for HomebrewRepository {
    fn get_all_packages(&self) -> Result<Vec<PackageInfo>> {
        // Return the pre-loaded installed packages immediately
        Ok(self.installed_packages.clone())
    }

    fn get_installed_packages(&self) -> Result<Vec<PackageInfo>> {
        // Since we only store installed packages, this is the same as get_all_packages
        self.get_all_packages()
    }

    fn search_packages(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let query_lower = query.to_lowercase();
        
        // Search in installed packages
        let results: Vec<PackageInfo> = self.installed_packages
            .iter()
            .filter(|pkg| {
                pkg.name.to_lowercase().contains(&query_lower)
                    || pkg.description.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect();
        
        Ok(results)
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
        let placeholder = self.installed_packages
            .iter()
            .find(|pkg| pkg.name == package_name);
            
        if let Some(pkg) = placeholder {
            if pkg.description == "Loading package information..." {
                // Request with HIGH PRIORITY for currently selected package
                self.request_package_details(package_name.to_string(), true);
                // Return the placeholder immediately for instant navigation
                return Some(pkg.clone());
            } else {
                // Return the existing detailed info
                return Some(pkg.clone());
            }
        }
        
        // If not found in installed packages, request with normal priority
        self.request_package_details(package_name.to_string(), false);
        None
    }

    fn install_package(&self, _package_name: &str) -> Result<()> {
        // TODO: Implement brew install command
        todo!("Implement actual Homebrew integration")
    }

    fn uninstall_package(&self, _package_name: &str) -> Result<()> {
        // TODO: Implement brew uninstall command
        todo!("Implement actual Homebrew integration")
    }

    fn update_package(&self, _package_name: &str) -> Result<()> {
        // TODO: Implement brew upgrade command
        todo!("Implement actual Homebrew integration")
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Convert a homebrew Formula to our PackageInfo structure
fn formula_to_package_info(formula: &Formula) -> PackageInfo {
    let installed_version = if formula.is_installed() {
        // Get the latest installed version if available
        formula.installed.first().map(|i| i.version.clone())
    } else {
        None
    };

    PackageInfo::new(
        formula.name.clone(),
        formula.desc.clone(),
        formula.homepage.clone(),
        formula.versions.stable.clone().unwrap_or_else(|| "unknown".to_string()),
        installed_version,
    )
}

/// Convert a homebrew Cask to our PackageInfo structure  
fn cask_to_package_info(cask: &Cask) -> PackageInfo {
    let installed_version = if cask.is_installed() {
        cask.installed.clone()
    } else {
        None
    };

    PackageInfo::new(
        cask.token.clone(),
        cask.desc.clone().unwrap_or_else(|| {
            if cask.name.is_empty() {
                "No description available".to_string()
            } else {
                cask.name.join(", ")
            }
        }),
        cask.homepage.clone(),
        cask.version.clone(),
        installed_version,
    )
}
