use crate::models::PackageInfo;
use crate::repository::PackageRepository;
use anyhow::Result;
use ratatui::widgets::ListState;
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
            repository,
        };
        app.list_state.select(Some(0));
        Ok(app)
    }

    /// Refreshes the package list from the repository
    pub fn refresh_packages(&mut self) -> Result<()> {
        self.items = self.repository.get_all_packages()?;
        self.apply_filter();
        Ok(())
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
}
