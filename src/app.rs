use crate::entities::package_info::PackageInfo;
use crate::repository::HomebrewRepository;
use anyhow::Result;
use ratatui::widgets::ListState;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Mock update stages for UX testing
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStage {
    Idle,
    Starting,
    Downloading,
    Installing,
    Completing,
    Finished,
    // Uninstall stages
    UninstallStarting,
    UninstallRemoving,
    UninstallCleaning,
    UninstallFinished,
}

/// Modal state for the application
#[derive(Debug, Clone, PartialEq)]
pub enum ModalState {
    None,
    UpdateProgress,
    UninstallConfirmation,
}

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
    pub pre_search_selection: Option<usize>, // Track selection before search started
    pub status_messages: VecDeque<(String, Instant)>,
    repository: HomebrewRepository,
    // Multi-column layout state
    pub current_columns: usize,
    pub rows_per_column: usize,
    pub column_scroll_offset: usize, // Track which column is the leftmost visible
    // Mock update state
    pub is_updating: bool,
    pub update_package_name: Option<String>,
    pub update_start_time: Option<Instant>,
    pub update_stage: UpdateStage,
    pub is_uninstalling: bool,    // Track if this is an uninstall operation
    pub real_update_called: bool, // Track if real update has been called
    pub pending_uninstall_package: Option<String>, // Package pending uninstall confirmation
    // Modal state
    pub modal_state: ModalState,
}

impl App {
    /// Creates a new application instance
    pub fn new(repository: HomebrewRepository) -> Result<Self> {
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
            pre_search_selection: None,
            status_messages: VecDeque::new(),
            repository,
            current_columns: 1,
            rows_per_column: 0,
            column_scroll_offset: 0,
            is_updating: false,
            update_package_name: None,
            update_start_time: None,
            update_stage: UpdateStage::Idle,
            is_uninstalling: false,
            real_update_called: false,
            pending_uninstall_package: None,
            modal_state: ModalState::None,
        };
        app.list_state.select(Some(0));
        Ok(app)
    }

    /// Refreshes the package list from the repository
    pub fn refresh_packages(&mut self) -> Result<()> {
        self.refresh_packages_with_selection(None)
    }

    /// Refreshes the package list from the repository, optionally preserving selection
    fn refresh_packages_with_selection(&mut self, preserve_selection: Option<usize>) -> Result<()> {
        // Use the new repository method to refresh all packages
        self.repository.refresh_all_packages()?;

        // Get the refreshed packages from the repository
        self.items = self.repository.get_all_packages()?;
        self.apply_filter_with_selection(preserve_selection);
        self.reset_column_scroll(); // Reset horizontal scrolling on refresh

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
        self.ensure_selection_visible();
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
        self.ensure_selection_visible();
        self.reset_scroll();
    }

    /// Resets the scroll position and updates last interaction time
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
        self.last_interaction = Instant::now();
    }

    /// Resets column scrolling to the beginning
    pub fn reset_column_scroll(&mut self) {
        self.column_scroll_offset = 0;
    }

    /// Ensures the currently selected item is visible by adjusting column scroll if needed
    fn ensure_selection_visible(&mut self) {
        // Use the cached layout information from the UI
        if self.current_columns <= 1 || self.rows_per_column == 0 {
            return; // No multi-column layout or layout not initialized yet
        }

        if let Some(selected_idx) = self.list_state.selected() {
            // Calculate which column the selected item is in
            let selected_column = selected_idx / self.rows_per_column;

            // Calculate the range of visible columns
            let leftmost_visible = self.column_scroll_offset;
            let rightmost_visible = self.column_scroll_offset + self.current_columns - 1;

            // Adjust scroll if selected column is not visible
            if selected_column < leftmost_visible {
                // Selected column is to the left of visible area
                self.column_scroll_offset = selected_column;
            } else if selected_column > rightmost_visible {
                // Selected column is to the right of visible area
                self.column_scroll_offset = selected_column - self.current_columns + 1;
            }
        }
    }

    /// Moves down by a page (10 items)
    pub fn page_down(&mut self) {
        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len == 0 {
            return;
        }

        let page_size = 10;
        let current = self.list_state.selected().unwrap_or(0);
        let new_index = std::cmp::min(current + page_size, items_len - 1);
        self.list_state.select(Some(new_index));
        self.ensure_selection_visible();
        self.reset_scroll();
    }

    /// Moves up by a page (10 items)
    pub fn page_up(&mut self) {
        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len == 0 {
            return;
        }

        let page_size = 10;
        let current = self.list_state.selected().unwrap_or(0);
        let new_index = current.saturating_sub(page_size);
        self.list_state.select(Some(new_index));
        self.ensure_selection_visible();
        self.reset_scroll();
    }

    /// Moves to the first item
    pub fn first(&mut self) {
        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len > 0 {
            self.list_state.select(Some(0));
            self.ensure_selection_visible();
            self.reset_scroll();
        }
    }

    /// Moves to the last item
    pub fn go_to_last(&mut self) {
        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len > 0 {
            self.list_state.select(Some(items_len - 1));
            self.ensure_selection_visible();
            self.reset_scroll();
        }
    }

    /// Updates the current layout information for multi-column navigation
    pub fn update_layout(&mut self, visible_columns: usize, rows_per_column: usize) {
        self.current_columns = visible_columns;
        self.rows_per_column = rows_per_column;
    }

    /// Moves left to the previous column (only makes sense in multi-column layout)
    pub fn move_left(&mut self) {
        if self.current_columns <= 1 {
            return; // No horizontal movement in single column
        }

        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len == 0 {
            return;
        }

        if let Some(current) = self.list_state.selected() {
            // Calculate current column and row (considering scroll offset)
            let current_col = current / self.rows_per_column;
            let current_row = current % self.rows_per_column;

            if current_col > 0 {
                // Move to previous column, same row
                let new_col = current_col - 1;
                let new_index = new_col * self.rows_per_column + current_row;

                // Make sure the new index is valid
                if new_index < items_len {
                    self.list_state.select(Some(new_index));
                    self.ensure_selection_visible();
                    self.reset_scroll();
                }
            }
        }
    }

    /// Moves right to the next column (only makes sense in multi-column layout)
    pub fn move_right(&mut self) {
        if self.current_columns <= 1 {
            return; // No horizontal movement in single column
        }

        let items_len = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        };

        if items_len == 0 {
            return;
        }

        if let Some(current) = self.list_state.selected() {
            // Calculate current column and row
            let current_col = current / self.rows_per_column;
            let current_row = current % self.rows_per_column;

            // Calculate total number of columns needed
            let total_columns = items_len.div_ceil(self.rows_per_column);

            if current_col < total_columns - 1 {
                // Move to next column, same row
                let new_col = current_col + 1;
                let new_index = new_col * self.rows_per_column + current_row;

                // Make sure the new index is valid
                if new_index < items_len {
                    self.list_state.select(Some(new_index));
                    self.ensure_selection_visible();
                    self.reset_scroll();
                }
            }
        }
    }

    /// Updates the horizontal scroll offset for long package names
    pub fn update_scroll(&mut self, available_width: usize) {
        let items = if self.is_searching {
            &self.filtered_items
        } else {
            &self.items
        };

        if let Some(selected) = self.list_state.selected()
            && selected < items.len()
        {
            let item_name = &items[selected].name;
            let name_width = item_name.len();

            // Only scroll if the name is longer than available width and 3 seconds have passed
            if name_width > available_width
                && self.last_interaction.elapsed() > Duration::from_secs(3)
            {
                let max_offset = name_width.saturating_sub(available_width);
                self.scroll_offset = (self.scroll_offset + 1) % (max_offset + available_width / 2);
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
        self.get_selected_package().cloned()
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
        // Save current selection before starting search
        self.pre_search_selection = self.list_state.selected();
        self.is_searching = true;
        self.search_query.clear();
        self.reset_column_scroll(); // Reset horizontal scrolling when starting search
        self.apply_filter();
    }

    /// Ends search mode and maintains selection of the currently selected item
    pub fn end_search(&mut self) {
        // Get the currently selected package from filtered results before ending search
        let selected_package_name = self.get_selected_package().map(|pkg| pkg.name.clone());

        self.is_searching = false;
        self.search_query.clear();

        // Find and select the same package in the full items list
        if let Some(package_name) = selected_package_name {
            // Find the index of this package in the full items list
            if let Some(index) = self.items.iter().position(|pkg| pkg.name == package_name) {
                self.list_state.select(Some(index));
            } else if !self.items.is_empty() {
                // Fallback to first item if package not found (shouldn't happen)
                self.list_state.select(Some(0));
            } else {
                self.list_state.select(None);
            }
        } else if !self.items.is_empty() {
            // No selection in search mode, select first item
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }

        self.column_scroll_offset = 0;
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
        self.apply_filter_with_selection(None);
    }

    fn apply_filter_with_selection(&mut self, preserve_selection: Option<usize>) {
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

        // Apply selection based on preservation request
        if let Some(target_index) = preserve_selection {
            // Preserve selection at the given index
            let max_index = if self.is_searching {
                self.filtered_items.len()
            } else {
                self.items.len()
            };

            if max_index > 0 {
                // Ensure index is within bounds
                let clamped_index = target_index.min(max_index - 1);
                self.list_state.select(Some(clamped_index));
            } else {
                self.list_state.select(None);
            }
        } else {
            // Default behavior: Reset selection to first item after filtering
            if !self.filtered_items.is_empty() {
                self.list_state.select(Some(0));
            } else {
                self.list_state.select(None);
            }
        }
        self.reset_scroll();
    }

    /// Uninstalls the currently selected package
    pub fn uninstall_selected_package(&mut self) -> Result<()> {
        if let Some(package) = self.get_selected_package() {
            if !self.is_updating {
                // Show confirmation modal instead of immediately uninstalling
                self.pending_uninstall_package = Some(package.name.clone());
                self.modal_state = ModalState::UninstallConfirmation;
            } else {
                self.add_status_message("Another operation is currently in progress".to_string());
            }
        }
        Ok(())
    }

    /// Updates the currently selected package (mock implementation for UX testing)
    pub fn update_selected_package(&mut self) -> Result<()> {
        if let Some(package) = self.get_selected_package() {
            if package.has_update_available() && !self.is_updating {
                // Start mock update process
                self.start_mock_update(package.name.clone());
            } else if !package.has_update_available() {
                self.add_status_message(format!("{} is already up to date", package.name));
            } else if self.is_updating {
                self.add_status_message("Another package is currently being updated".to_string());
            }
        }
        Ok(())
    }

    /// Starts a mock update process
    fn start_mock_update(&mut self, package_name: String) {
        // Start the UI mock progression immediately for better UX
        self.is_updating = true;
        self.update_package_name = Some(package_name.clone());
        self.update_start_time = Some(Instant::now());
        self.update_stage = UpdateStage::Starting;
        self.real_update_called = false;
        self.modal_state = ModalState::UpdateProgress;
        self.add_status_message(format!("Starting update for {}", package_name));

        // The real update will be called during the "Installing" stage
        // to better simulate the actual timing of when brew upgrade runs
    }

    /// Starts a mock uninstall process
    fn start_mock_uninstall(&mut self, package_name: String) {
        // Start the UI mock progression immediately for better UX
        self.is_updating = true;
        self.is_uninstalling = true;
        self.update_package_name = Some(package_name.clone());
        self.update_start_time = Some(Instant::now());
        self.update_stage = UpdateStage::UninstallStarting;
        self.real_update_called = false; // Track if real uninstall has been called
        self.modal_state = ModalState::UpdateProgress;
        self.add_status_message(format!("Starting uninstall for {}", package_name));

        // The real uninstall will be called during the "UninstallRemoving" stage
    }

    /// Updates the mock update progress (call this regularly to simulate progress)
    pub fn update_mock_progress(&mut self) {
        if !self.is_updating {
            return;
        }

        let elapsed = self
            .update_start_time
            .map(|start| start.elapsed())
            .unwrap_or_default();

        let package_name = self.update_package_name.as_ref().unwrap();

        match self.update_stage {
            UpdateStage::Starting if elapsed > Duration::from_millis(800) => {
                self.update_stage = UpdateStage::Downloading;
                self.add_status_message(format!("Downloading {} updates...", package_name));
            }
            UpdateStage::Downloading if elapsed > Duration::from_millis(2500) => {
                self.update_stage = UpdateStage::Installing;
                self.add_status_message(format!("Installing {} updates...", package_name));
            }
            UpdateStage::Installing if elapsed > Duration::from_millis(4000) => {
                // Call real update during Installing stage if not called yet
                if !self.real_update_called && !self.is_uninstalling {
                    if let Err(e) = self.repository.update_package(package_name) {
                        self.add_status_message(format!(
                            "❌ Failed to update {}: {}",
                            package_name, e
                        ));
                        self.finish_mock_update();
                        return;
                    }
                    self.real_update_called = true;
                }

                self.update_stage = UpdateStage::Completing;
                self.add_status_message(format!("Completing {} installation...", package_name));
            }
            UpdateStage::Completing if elapsed > Duration::from_millis(5000) => {
                self.update_stage = UpdateStage::Finished;
                self.add_status_message(format!("✅ {} updated successfully!", package_name));
            }
            UpdateStage::Finished if elapsed > Duration::from_millis(6000) => {
                // Reset update state
                self.finish_mock_update();
            }
            // Uninstall stages
            UpdateStage::UninstallStarting if elapsed > Duration::from_millis(500) => {
                self.update_stage = UpdateStage::UninstallRemoving;
                self.add_status_message(format!("Removing {} files...", package_name));
            }
            UpdateStage::UninstallRemoving if elapsed > Duration::from_millis(2000) => {
                // Call real uninstall during UninstallRemoving stage if not called yet
                if !self.real_update_called && self.is_uninstalling {
                    if let Err(e) = self.repository.uninstall_package(package_name) {
                        self.add_status_message(format!(
                            "❌ Failed to uninstall {}: {}",
                            package_name, e
                        ));
                        self.finish_mock_uninstall();
                        return;
                    }
                    self.real_update_called = true;
                }

                self.update_stage = UpdateStage::UninstallCleaning;
                self.add_status_message(format!("Cleaning up {} dependencies...", package_name));
            }
            UpdateStage::UninstallCleaning if elapsed > Duration::from_millis(3500) => {
                self.update_stage = UpdateStage::UninstallFinished;
                self.add_status_message(format!("✅ {} uninstalled successfully!", package_name));
            }
            UpdateStage::UninstallFinished if elapsed > Duration::from_millis(4500) => {
                // Reset uninstall state and remove from list
                self.finish_mock_uninstall();
            }
            _ => {}
        }
    }

    /// Finishes the mock uninstall and removes package from list
    fn finish_mock_uninstall(&mut self) {
        let package_name = self.update_package_name.clone();

        // Save current selection before making changes
        let current_selection = self.list_state.selected();

        self.is_updating = false;
        self.is_uninstalling = false;
        self.real_update_called = false;
        self.update_package_name = None;
        self.update_start_time = None;
        self.update_stage = UpdateStage::Idle;
        self.modal_state = ModalState::None;

        // Remove package from list after uninstall
        if let Some(name) = package_name {
            // Clear the package from repository cache to prevent reappearance
            self.repository.clear_package_cache(&name);

            // Remove from our package lists immediately since uninstall was successful
            self.items.retain(|p| p.name != name);
            if self.is_searching {
                self.filtered_items.retain(|p| p.name != name);
            }

            // Calculate new selection position: move to the item above the deleted one
            // If the deleted item was at index 0, stay at 0
            // Otherwise, move to index - 1
            let new_selection = if let Some(selected) = current_selection {
                if selected > 0 {
                    Some(selected - 1)
                } else {
                    Some(0)
                }
            } else {
                None
            };

            // Refresh the entire package list to ensure consistency
            // and apply the new selection
            if let Err(e) = self.refresh_packages_with_selection(new_selection) {
                self.add_status_message(format!("⚠️  Failed to refresh package list: {}", e));
            }

            self.add_status_message(format!("✅ Successfully uninstalled {}", name));
        }
    }

    /// Finishes the mock update and resets state
    fn finish_mock_update(&mut self) {
        let package_name = self.update_package_name.clone();

        // Save current selection before refreshing
        let current_selection = self.list_state.selected();

        self.is_updating = false;
        self.is_uninstalling = false;
        self.real_update_called = false;
        self.update_package_name = None;
        self.update_start_time = None;
        self.update_stage = UpdateStage::Idle;
        self.modal_state = ModalState::None;

        // Refresh package list after update to ensure all metadata is current
        if let Some(name) = package_name {
            // Add a small delay to ensure brew has updated its internal state
            std::thread::sleep(Duration::from_millis(500));

            // Try to refresh the specific package first
            if let Err(e) = self.refresh_single_package(name.clone()) {
                self.add_status_message(format!("⚠️  Failed to refresh {}: {}", name, e));
            }

            // Also refresh the entire package list to ensure consistency,
            // preserving the cursor position on the updated package
            if let Err(e) = self.refresh_packages_with_selection(current_selection) {
                self.add_status_message(format!("⚠️  Failed to refresh package list: {}", e));
            }
        }
    }

    /// Gets the current update status message for display
    pub fn get_update_status(&self) -> Option<String> {
        if !self.is_updating {
            return None;
        }

        let package_name = self.update_package_name.as_ref()?;
        let elapsed = self.update_start_time?.elapsed();

        match self.update_stage {
            UpdateStage::Starting => Some(format!("🔄 Preparing to update {}...", package_name)),
            UpdateStage::Downloading => {
                let dots = ".".repeat(((elapsed.as_millis() / 300) % 4) as usize);
                Some(format!("⬇️  Downloading {} updates{}", package_name, dots))
            }
            UpdateStage::Installing => {
                let dots = ".".repeat(((elapsed.as_millis() / 200) % 4) as usize);
                Some(format!("🔧 Installing {} updates{}", package_name, dots))
            }
            UpdateStage::Completing => {
                Some(format!("✨ Finalizing {} installation...", package_name))
            }
            UpdateStage::Finished => Some(format!("✅ {} updated successfully!", package_name)),
            // Uninstall status messages
            UpdateStage::UninstallStarting => {
                Some(format!("🗑️  Preparing to uninstall {}...", package_name))
            }
            UpdateStage::UninstallRemoving => {
                let dots = ".".repeat(((elapsed.as_millis() / 200) % 4) as usize);
                Some(format!("🗂️  Removing {} files{}", package_name, dots))
            }
            UpdateStage::UninstallCleaning => {
                let dots = ".".repeat(((elapsed.as_millis() / 300) % 4) as usize);
                Some(format!(
                    "🧹 Cleaning up {} dependencies{}",
                    package_name, dots
                ))
            }
            UpdateStage::UninstallFinished => {
                Some(format!("✅ {} uninstalled successfully!", package_name))
            }
            UpdateStage::Idle => None,
        }
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

    /// Confirms the uninstall operation
    pub fn confirm_uninstall(&mut self) {
        if let Some(package_name) = self.pending_uninstall_package.take() {
            self.modal_state = ModalState::None;
            self.start_mock_uninstall(package_name);
        }
    }

    /// Cancels the uninstall operation
    pub fn cancel_uninstall(&mut self) {
        self.pending_uninstall_package = None;
        self.modal_state = ModalState::None;
        self.add_status_message("Uninstall cancelled".to_string());
    }

    /// Refreshes metadata for a single package after update
    fn refresh_single_package(&mut self, package_name: String) -> Result<()> {
        match self.repository.refresh_package(&package_name) {
            Ok(Some(updated_package)) => {
                // Update the package in our main list
                if let Some(index) = self.items.iter().position(|p| p.name == package_name) {
                    self.items[index] = updated_package.clone();
                }

                // Update the package in filtered list if we're searching
                if self.is_searching
                    && let Some(index) = self
                        .filtered_items
                        .iter()
                        .position(|p| p.name == package_name)
                {
                    self.filtered_items[index] = updated_package;
                }

                self.add_status_message(format!("📦 Refreshed metadata for {}", package_name));
            }
            Ok(None) => {
                // Package not found (maybe uninstalled)
                self.items.retain(|p| p.name != package_name);
                if self.is_searching {
                    self.filtered_items.retain(|p| p.name != package_name);
                }
                self.add_status_message(format!("📦 {} no longer found", package_name));
            }
            Err(e) => {
                return Err(e);
            }
        }
        Ok(())
    }
}
