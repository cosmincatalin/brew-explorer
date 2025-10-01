use crate::models::PackageInfo;
use crate::repository::PackageRepository;
use anyhow::Result;
use ratatui::widgets::ListState;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Application state and business logic
pub struct App {
    pub items: Vec<PackageInfo>,
    pub list_state: ListState,
    pub scroll_offset: usize,
    pub last_interaction: Instant,
    pub should_quit: bool,
    pub search_query: String,
    pub filtered_items: Vec<PackageInfo>,
    pub is_searching: bool,
    pub status_messages: VecDeque<(String, Instant)>,
    repository: Box<dyn PackageRepository>,
}

impl App {
    /// Creates a new application instance
    pub fn new(repository: Box<dyn PackageRepository>) -> Result<Self> {
        let items = repository.get_all_packages()?;
        let mut app = Self {
            filtered_items: items.clone(),
            items,
            list_state: ListState::default(),
            scroll_offset: 0,
            last_interaction: Instant::now(),
            should_quit: false,
            search_query: String::new(),
            is_searching: false,
            status_messages: VecDeque::new(),
            repository,
        };
        app.list_state.select(Some(0));
        Ok(app)
    }

    /// Refreshes the package list from the repository
    pub fn refresh_packages(&mut self) -> Result<()> {
        self.items = self.repository.get_all_packages()?;
        self.apply_filter();
        
        // Check for status updates from repository if it's a HomebrewRepository
        if let Some(status) = self.get_repository_status() {
            self.add_status_message(status);
        }
        
        Ok(())
    }
    
    /// Gets status from repository if available
    fn get_repository_status(&self) -> Option<String> {
        // This is a bit hacky, but we need to downcast to HomebrewRepository
        // In a real application, you might want to add status methods to the trait
        if let Some(homebrew_repo) = self.repository.as_any().downcast_ref::<crate::repository::HomebrewRepository>() {
            homebrew_repo.get_current_status()
        } else {
            None
        }
    }

    /// Moves to the next item in the list
    pub fn next(&mut self) {
        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= items_len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.reset_scroll();
    }

    /// Moves to the previous item in the list
    pub fn previous(&mut self) {
        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    items_len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.reset_scroll();
    }

    /// Resets the scroll position and updates last interaction time
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
        self.last_interaction = Instant::now();
    }

    /// Updates the horizontal scroll offset for long package names
    pub fn update_scroll(&mut self, available_width: usize) {
        let items = if self.is_searching {
            &self.filtered_items
        } else {
            &self.items
        };

        if let Some(selected) = self.list_state.selected() {
            if selected < items.len() {
                let item_name = &items[selected].name;
                let name_width = item_name.len();

                // Only scroll if the name is longer than available width and 3 seconds have passed
                if name_width > available_width
                    && self.last_interaction.elapsed() > Duration::from_secs(3)
                {
                    let max_offset = name_width.saturating_sub(available_width);
                    self.scroll_offset =
                        (self.scroll_offset + 1) % (max_offset + available_width / 2);
                }
            }
        }
    }

    /// Gets the currently selected package
    pub fn get_selected_package(&self) -> Option<&PackageInfo> {
        let items = if self.is_searching {
            &self.filtered_items
        } else {
            &self.items
        };

        self.list_state.selected().and_then(|i| items.get(i))
    }

    /// Gets the currently selected package with full details (fetches if needed)
    pub fn get_selected_package_details(&self) -> Option<PackageInfo> {
        if let Some(package) = self.get_selected_package() {
            // Try to get detailed info from repository
            if let Some(detailed_package) = self.repository.get_package_details(&package.name) {
                return Some(detailed_package);
            }
            // If no detailed info available, return the placeholder
            Some(package.clone())
        } else {
            None
        }
    }
    
    /// Update package details from cache if available (for background loading)
    pub fn update_package_details(&mut self) {
        // Check for loading animation updates
        if let Some(homebrew_repo) = self.repository.as_any().downcast_ref::<crate::repository::HomebrewRepository>() {
            if homebrew_repo.update_loading_animations() {
                // Animation state changed, we might want to refresh if showing loading text
                // The UI will automatically get the updated text when it calls get_selected_package_details
            }
        }
        
        if let Some(selected_package) = self.get_selected_package() {
            // Check if there are updated details available
            if let Some(homebrew_repo) = self.repository.as_any().downcast_ref::<crate::repository::HomebrewRepository>() {
                if homebrew_repo.has_updated_details(&selected_package.name) {
                    if let Some(_updated_details) = homebrew_repo.get_cached_details(&selected_package.name) {
                        // Details have been updated - no need to do anything as they're already cached
                    }
                }
            }
        }
    }

    /// Gets the current list of packages to display
    pub fn get_display_items(&self) -> &Vec<PackageInfo> {
        if self.is_searching {
            &self.filtered_items
        } else {
            &self.items
        }
    }

    /// Starts search mode
    pub fn start_search(&mut self) {
        self.is_searching = true;
        self.search_query.clear();
        self.apply_filter();
    }

    /// Ends search mode
    pub fn end_search(&mut self) {
        self.is_searching = false;
        self.search_query.clear();
        self.list_state.select(Some(0));
        self.reset_scroll();
    }

    /// Adds a character to the search query
    pub fn add_search_char(&mut self, c: char) {
        if self.is_searching {
            self.search_query.push(c);
            self.apply_filter();
        }
    }

    /// Removes the last character from the search query
    pub fn remove_search_char(&mut self) {
        if self.is_searching && !self.search_query.is_empty() {
            self.search_query.pop();
            self.apply_filter();
        }
    }

    /// Applies the current search filter
    fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_items = self.items.clone();
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_items = self
                .items
                .iter()
                .filter(|pkg| {
                    pkg.name.to_lowercase().contains(&query_lower)
                        || pkg.description.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect();
        }

        // Reset selection to first item after filtering
        if !self.filtered_items.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
        self.reset_scroll();
    }

    /// Installs the currently selected package
    pub fn install_selected_package(&mut self) -> Result<()> {
        if let Some(package) = self.get_selected_package() {
            if !package.is_installed() {
                self.repository.install_package(&package.name)?;
                self.refresh_packages()?;
            }
        }
        Ok(())
    }

    /// Uninstalls the currently selected package
    pub fn uninstall_selected_package(&mut self) -> Result<()> {
        if let Some(package) = self.get_selected_package() {
            if package.is_installed() {
                self.repository.uninstall_package(&package.name)?;
                self.refresh_packages()?;
            }
        }
        Ok(())
    }

    /// Updates the currently selected package
    pub fn update_selected_package(&mut self) -> Result<()> {
        if let Some(package) = self.get_selected_package() {
            if package.has_update_available() {
                self.repository.update_package(&package.name)?;
                self.refresh_packages()?;
            }
        }
        Ok(())
    }

    /// Sets the quit flag
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Adds a status message that will be displayed in the status bar
    pub fn add_status_message(&mut self, message: String) {
        self.status_messages.push_back((message, Instant::now()));
        // Keep only the last 5 messages
        while self.status_messages.len() > 5 {
            self.status_messages.pop_front();
        }
    }

    /// Gets the current status message to display
    pub fn get_current_status(&mut self) -> Option<String> {
        // Clean up old messages (older than 10 seconds)
        let now = Instant::now();
        while let Some((_, timestamp)) = self.status_messages.front() {
            if now.duration_since(*timestamp) > Duration::from_secs(10) {
                self.status_messages.pop_front();
            } else {
                break;
            }
        }

        // Return the most recent message
        self.status_messages.back().map(|(msg, _)| msg.clone())
    }

    /// Updates repository status if available (called periodically from main loop)
    pub fn update_repository_status(&mut self) {
        if let Some(status) = self.get_repository_status() {
            self.add_status_message(status);
        }
    }
    
    /// Refresh package list to pick up newly cached packages
    pub fn refresh_package_list(&mut self) -> Result<()> {
        let current_selection = self.list_state.selected();
        let current_search = self.search_query.clone();
        
        // Get updated packages from repository
        self.items = self.repository.get_all_packages()?;
        
        // Reapply search filter if we're in search mode
        if self.is_searching {
            self.search_query = current_search;
            self.apply_filter();
        }
        
        // Restore selection if possible
        if let Some(selection) = current_selection {
            let max_index = if self.is_searching {
                self.filtered_items.len()
            } else {
                self.items.len()
            };
            
            if max_index > 0 {
                self.list_state.select(Some(selection.min(max_index - 1)));
            }
        }
        
        Ok(())
    }
}
