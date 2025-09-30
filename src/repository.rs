use crate::models::PackageInfo;
use anyhow::Result;

/// Trait for package repository operations
pub trait PackageRepository {
    fn get_all_packages(&self) -> Result<Vec<PackageInfo>>;
    fn get_installed_packages(&self) -> Result<Vec<PackageInfo>>;
    fn search_packages(&self, query: &str) -> Result<Vec<PackageInfo>>;
    fn install_package(&self, package_name: &str) -> Result<()>;
    fn uninstall_package(&self, package_name: &str) -> Result<()>;
    fn update_package(&self, package_name: &str) -> Result<()>;
}

/// Mock implementation for testing and development
pub struct MockPackageRepository {
    packages: Vec<PackageInfo>,
}

impl MockPackageRepository {
    pub fn new() -> Self {
        Self {
            packages: Self::generate_sample_packages(),
        }
    }

    fn generate_sample_packages() -> Vec<PackageInfo> {
        vec![
            PackageInfo::new(
                "git".to_string(),
                "Distributed revision control system".to_string(),
                "https://git-scm.com".to_string(),
                "2.42.0".to_string(),
                Some("2.42.0".to_string()),
            ),
            PackageInfo::new(
                "node".to_string(),
                "Platform built on V8 to build network applications".to_string(),
                "https://nodejs.org/".to_string(),
                "20.8.0".to_string(),
                Some("18.17.1".to_string()),
            ),
            PackageInfo::new(
                "very-long-package-name-that-should-scroll-horizontally".to_string(),
                "This is a package with an extremely long name to demonstrate horizontal scrolling functionality in the TUI interface".to_string(),
                "https://example.com/very-long-package".to_string(),
                "1.0.0".to_string(),
                None,
            ),
            PackageInfo::new(
                "python@3.11".to_string(),
                "Interpreted, interactive, object-oriented programming language".to_string(),
                "https://www.python.org/".to_string(),
                "3.11.5".to_string(),
                Some("3.11.5".to_string()),
            ),
            PackageInfo::new(
                "rust".to_string(),
                "Safe, concurrent, practical language".to_string(),
                "https://www.rust-lang.org/".to_string(),
                "1.72.0".to_string(),
                Some("1.71.0".to_string()),
            ),
        ]
    }
}

impl PackageRepository for MockPackageRepository {
    fn get_all_packages(&self) -> Result<Vec<PackageInfo>> {
        Ok(self.packages.clone())
    }

    fn get_installed_packages(&self) -> Result<Vec<PackageInfo>> {
        Ok(self
            .packages
            .iter()
            .filter(|pkg| pkg.is_installed())
            .cloned()
            .collect())
    }

    fn search_packages(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let query_lower = query.to_lowercase();
        Ok(self
            .packages
            .iter()
            .filter(|pkg| {
                pkg.name.to_lowercase().contains(&query_lower)
                    || pkg.description.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect())
    }

    fn install_package(&self, _package_name: &str) -> Result<()> {
        // Mock implementation - in real version would call brew install
        Ok(())
    }

    fn uninstall_package(&self, _package_name: &str) -> Result<()> {
        // Mock implementation - in real version would call brew uninstall
        Ok(())
    }

    fn update_package(&self, _package_name: &str) -> Result<()> {
        // Mock implementation - in real version would call brew upgrade
        Ok(())
    }
}

/// Future implementation for actual Homebrew integration
pub struct HomebrewRepository;

impl HomebrewRepository {
    pub fn new() -> Self {
        Self
    }
}

impl PackageRepository for HomebrewRepository {
    fn get_all_packages(&self) -> Result<Vec<PackageInfo>> {
        // TODO: Implement actual brew command execution
        todo!("Implement actual Homebrew integration")
    }

    fn get_installed_packages(&self) -> Result<Vec<PackageInfo>> {
        // TODO: Implement brew list command
        todo!("Implement actual Homebrew integration")
    }

    fn search_packages(&self, _query: &str) -> Result<Vec<PackageInfo>> {
        // TODO: Implement brew search command
        todo!("Implement actual Homebrew integration")
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
}
