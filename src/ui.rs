use crate::app::{App, ModalState, UpdateStage};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Wrap},
};
use std::time::Duration;

/// Renders a fancy loading screen with ASCII art
pub fn render_loading_screen(f: &mut Frame, loading_dots: usize, elapsed: std::time::Duration) {
    let area = f.area();

    // ASCII art for "Brew Explorer"
    let ascii_art = vec![
        "  ██████╗ ██████╗ ███████╗██╗    ██╗",
        "  ██╔══██╗██╔══██╗██╔════╝██║    ██║",
        "  ██████╔╝██████╔╝█████╗  ██║ █╗ ██║",
        "  ██╔══██╗██╔══██╗██╔══╝  ██║███╗██║",
        "  ██████╔╝██║  ██║███████╗╚███╔███╔╝",
        "  ╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚══╝╚══╝ ",
        "",
        "███████╗██╗  ██╗██████╗ ██╗      ██████╗ ██████╗ ███████╗██████╗ ",
        "██╔════╝╚██╗██╔╝██╔══██╗██║     ██╔═══██╗██╔══██╗██╔════╝██╔══██╗",
        "█████╗   ╚███╔╝ ██████╔╝██║     ██║   ██║██████╔╝█████╗  ██████╔╝",
        "██╔══╝   ██╔██╗ ██╔═══╝ ██║     ██║   ██║██╔══██╗██╔══╝  ██╔══██╗",
        "███████╗██╔╝ ██╗██║     ███████╗╚██████╔╝██║  ██║███████╗██║  ██║",
        "╚══════╝╚═╝  ╚═╝╚═╝     ╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝",
    ];

    // Create centered layout
    let vertical_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(ascii_art.len() as u16),
            Constraint::Length(5),
            Constraint::Percentage(25),
        ])
        .split(area);

    // ASCII art block
    let ascii_lines: Vec<Line> = ascii_art
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                *line,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    let ascii_block = Paragraph::new(ascii_lines)
        .alignment(Alignment::Center)
        .block(Block::default());

    f.render_widget(ascii_block, vertical_layout[1]);

    // Loading message with animated dots
    let dots = match loading_dots {
        0 => "   ",
        1 => ".  ",
        2 => ".. ",
        3 => "...",
        _ => "   ", // Reset to empty for smoother animation
    };

    // Calculate elapsed seconds for display
    let elapsed_secs = elapsed.as_secs();
    let elapsed_display = if elapsed_secs > 0 {
        format!(" ({}s)", elapsed_secs)
    } else {
        String::new()
    };

    let loading_text = vec![
        Line::from(vec![
            Span::styled("🍺 ", Style::default()),
            Span::styled(
                "Loading Homebrew packages",
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                dots,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(elapsed_display, Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press 'q' to quit",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let loading_block = Paragraph::new(loading_text)
        .alignment(Alignment::Center)
        .block(Block::default());

    f.render_widget(loading_block, vertical_layout[2]);
}

/// Renders the main UI
pub fn render_ui(f: &mut Frame, app: &mut App) {
    // Create main layout with status bar at the bottom
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    // Split the main area horizontally for package list and details
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[0]);

    render_package_list(f, app, content_chunks[0]);
    render_package_details(f, app, content_chunks[1]);
    render_status_bar(f, app, main_chunks[1]);

    // Render modal if one is open
    if app.modal_state != ModalState::None {
        render_modal(f, app);
    }
}

/// Renders the package list on the left panel with dynamic columns
fn render_package_list(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let available_width = area.width.saturating_sub(4) as usize; // Account for borders

    // Calculate optimal number of columns based on available width
    // Assume minimum 25 characters per package name + 3 characters padding
    let min_column_width = 28;
    let max_visible_columns = (available_width / min_column_width).clamp(1, 4); // Cap at 4 columns for readability

    let total_items = if app.is_searching {
        app.filtered_items.len()
    } else {
        app.items.len()
    };

    // Create title
    let title = if app.is_searching {
        if total_items == 0 {
            format!("Packages (Search: {}) - No results", app.search_query)
        } else {
            format!("Packages (Search: {})", app.search_query)
        }
    } else {
        "Packages".to_string()
    };

    if total_items == 0 {
        // Render empty list with message
        let empty_list = List::new(Vec::<ListItem>::new())
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_stateful_widget(empty_list, area, &mut app.list_state);
        return;
    }

    // Calculate the ideal rows per column for good distribution
    let ideal_rows_per_column = (area.height.saturating_sub(3)) as usize; // Account for borders and title
    let ideal_rows_per_column = ideal_rows_per_column.max(1);

    // Calculate total columns needed to display all items
    let total_columns_needed = total_items.div_ceil(ideal_rows_per_column);

    // Determine how many columns we can actually show
    let visible_columns = max_visible_columns.min(total_columns_needed);
    let rows_per_column = if total_columns_needed <= visible_columns {
        // All columns fit, distribute items evenly
        total_items.div_ceil(visible_columns)
    } else {
        // More columns than can fit, use ideal row count
        ideal_rows_per_column
    };

    // Update the app's layout information for navigation
    app.update_layout(visible_columns, rows_per_column);

    let items = app.get_display_items();

    if visible_columns == 1 {
        // Fall back to single column list for narrow spaces
        let list_items: Vec<ListItem> = items
            .iter()
            .enumerate()
            .map(|(i, package)| {
                let display_name = package.get_display_name();
                let content = if Some(i) == app.list_state.selected() {
                    apply_horizontal_scroll(&display_name, available_width, app)
                } else {
                    display_name
                };

                let style = get_package_style(package);
                ListItem::new(Line::from(Span::styled(content, style)))
            })
            .collect();

        let items_list = List::new(list_items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(items_list, area, &mut app.list_state);
    } else {
        // Multi-column layout using table
        let column_width = available_width / visible_columns;

        // Calculate total columns needed
        let total_columns = total_columns_needed;

        // Create column constraints - equal width for all columns
        let constraints: Vec<Constraint> = (0..visible_columns)
            .map(|_| Constraint::Percentage((100 / visible_columns) as u16))
            .collect();

        // Build rows for the table
        let mut table_rows: Vec<Row> = Vec::new();

        // Debug: check current selection
        let selected_idx = app.list_state.selected();

        for row_idx in 0..rows_per_column {
            let mut cells: Vec<Span> = Vec::new();

            for visible_col_idx in 0..visible_columns {
                // Calculate the actual column index considering scroll offset
                let actual_col_idx = app.column_scroll_offset + visible_col_idx;
                let item_idx = actual_col_idx * rows_per_column + row_idx;

                if item_idx < items.len() && actual_col_idx < total_columns {
                    let package = &items[item_idx];
                    let display_name = package.get_display_name();

                    // Truncate name to fit column width (Unicode-safe)
                    let truncated_name = if display_name.chars().count() > column_width - 2 {
                        let chars: Vec<char> = display_name.chars().collect();
                        let truncate_at = column_width.saturating_sub(3);
                        format!("{}…", chars[..truncate_at].iter().collect::<String>())
                    } else {
                        display_name
                    };

                    let style = get_package_style(package);

                    // Check if this item is selected
                    let is_selected = selected_idx == Some(item_idx);
                    let final_style = if is_selected {
                        style.bg(Color::Blue).add_modifier(Modifier::BOLD)
                    } else {
                        style
                    };

                    let prefix = if is_selected { ">> " } else { "   " };
                    cells.push(Span::styled(
                        format!("{}{}", prefix, truncated_name),
                        final_style,
                    ));
                } else {
                    // Empty cell for alignment
                    cells.push(Span::raw(""));
                }
            }

            table_rows.push(Row::new(cells));
        }

        let table = Table::new(table_rows, constraints)
            .block(Block::default().borders(Borders::ALL).title(title))
            .column_spacing(1);

        f.render_widget(table, area);
    }
}

/// Gets the appropriate style for a package based on its status
fn get_package_style(package: &crate::entities::package_info::PackageInfo) -> Style {
    if package.outdated || package.has_update_available() {
        // Use a more visible reddish color for packages with updates available
        Style::default().fg(Color::Rgb(220, 80, 80)) // Soft reddish color
    } else {
        // All packages are installed (since they come from brew --installed)
        Style::default().fg(Color::Green)
    }
}

/// Applies horizontal scrolling to a package name
fn apply_horizontal_scroll(name: &str, available_width: usize, app: &App) -> String {
    let chars: Vec<char> = name.chars().collect();
    let name_len = chars.len();

    if name_len > available_width && app.last_interaction.elapsed() > Duration::from_secs(3) {
        let start = app.scroll_offset % name_len.max(1);
        let end = (start + available_width).min(name_len);

        if start < name_len {
            if end <= name_len {
                chars[start..end].iter().collect()
            } else {
                // Wrap around
                let first_part: String = chars[start..].iter().collect();
                let second_part: String = chars[..end - name_len].iter().collect();
                format!("{}{}", first_part, second_part)
            }
        } else {
            name.to_string()
        }
    } else {
        name.to_string()
    }
}

/// Renders the package details on the right panel
fn render_package_details(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let package_details = app.get_selected_package_details();
    let details = match package_details.as_ref() {
        Some(package) => create_package_details_text(package),
        None => Text::from("No package selected"),
    };

    let details_paragraph = Paragraph::new(details)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Package Details"),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(details_paragraph, area);

    // Render help text at the bottom
    render_help_text(f, area);
}

/// Creates the detailed text for a package
fn create_package_details_text(package: &crate::entities::package_info::PackageInfo) -> Text<'_> {
    let installed_status = package.installation_status();
    let status_colour = if package.outdated || package.has_update_available() {
        // Use the same reddish color for packages with updates available
        Color::Rgb(220, 80, 80) // Soft reddish color
    } else {
        // All packages are installed (since they come from brew --installed)
        Color::Green
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&package.name),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Description: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(&package.description),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tap: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(package.tap.as_deref().unwrap_or("unknown")),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Caveats: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(package.caveats.as_deref().unwrap_or("none")),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Homepage: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&package.homepage, Style::default().fg(Color::Blue)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Installed Version: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(installed_status, Style::default().fg(status_colour)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Current Version: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(&package.current_version),
        ]),
        Line::from(""),
    ];

    // Add installation time if available
    if let Some(time_ago) = package.installed_ago() {
        lines.push(Line::from(vec![
            Span::styled("Installed: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(time_ago, Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));

    // Add the action hints as separate lines
    lines.extend(create_action_hints(package));

    Text::from(lines)
}

/// Creates action hints based on package state
fn create_action_hints(package: &crate::entities::package_info::PackageInfo) -> Vec<Line<'_>> {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            "⚡ Actions:",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )]),
        Line::from(""),
    ];

    // All packages are installed (since they come from brew --installed)
    // Add uninstall action
    lines.push(Line::from(vec![
        Span::raw("    ◦ "),
        Span::styled(
            "uninstall",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (press 'x' to remove)", Style::default().fg(Color::Gray)),
    ]));

    // Only add update action if update is available
    if package.has_update_available() {
        lines.push(Line::from(vec![
            Span::raw("    ◦ "),
            Span::styled(
                "update",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" (press 'u' to update)", Style::default().fg(Color::Gray)),
        ]));
    }

    lines
}

/// Renders help text at the bottom of the details panel
fn render_help_text(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = "Navigate: ↑/↓ ←/→ | Search: / | Actions: u/x | Quit: q";
    let help_paragraph = Paragraph::new(help_text).style(Style::default().fg(Color::Gray));

    let help_area = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    let help_y = help_area.bottom().saturating_sub(1);
    let help_rect = ratatui::layout::Rect {
        x: help_area.x,
        y: help_y,
        width: help_area.width,
        height: 1,
    };

    f.render_widget(help_paragraph, help_rect);
}

/// Renders the status bar at the bottom of the screen
fn render_status_bar(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let status_text = if let Some(update_status) = app.get_update_status() {
        // Prioritize update status when an update is in progress
        update_status
    } else if let Some(message) = app.get_current_status() {
        message
    } else {
        "Navigate: ↑/↓ ←/→ PgUp/PgDn Home/End | Search: / | Actions: u/x | Quit: q".to_string()
    };

    let status_paragraph = Paragraph::new(status_text)
        .style(Style::default().bg(Color::Blue).fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(status_paragraph, area);
}

/// Renders modal windows
fn render_modal(f: &mut Frame, app: &App) {
    match app.modal_state {
        ModalState::UpdateProgress => render_update_modal(f, app),
        ModalState::UninstallConfirmation => render_uninstall_confirmation_modal(f, app),
        ModalState::None => {}
    }
}

/// Renders the update progress modal
fn render_update_modal(f: &mut Frame, app: &App) {
    let area = f.area();

    // Create a centered modal area
    let modal_width = 60;
    let modal_height = 12;
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = ratatui::layout::Rect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    };

    // Clear the area behind the modal
    f.render_widget(Clear, modal_area);

    // Get update information
    let package_name = app
        .update_package_name
        .as_deref()
        .unwrap_or("Unknown Package");
    let elapsed = app
        .update_start_time
        .map(|start| start.elapsed())
        .unwrap_or_default();

    // Calculate progress based on stage and timing
    let (progress, stage_text, details, modal_title) = match app.update_stage {
        UpdateStage::Idle => (0, "Idle", "No operation in progress", "No Operation"),
        UpdateStage::Starting => (10, "Starting", "Preparing update process...", "Updating"),
        UpdateStage::Downloading => {
            let base_progress = 20;
            let additional = ((elapsed.as_millis() - 800) / 17).min(40) as u16; // Up to 40% more
            (
                base_progress + additional,
                "Downloading",
                "Fetching update files...",
                "Updating",
            )
        }
        UpdateStage::Installing => {
            let base_progress = 60;
            let additional = ((elapsed.as_millis() - 2500) / 15).min(25) as u16; // Up to 25% more
            (
                base_progress + additional,
                "Installing",
                "Installing new version...",
                "Updating",
            )
        }
        UpdateStage::Completing => (90, "Completing", "Finalizing installation...", "Updating"),
        UpdateStage::Finished => (
            100,
            "Complete",
            "Update completed successfully! Closing...",
            "Updating",
        ),
        // Uninstall stages
        UpdateStage::UninstallStarting => (
            15,
            "Starting",
            "Preparing uninstall process...",
            "Uninstalling",
        ),
        UpdateStage::UninstallRemoving => {
            let base_progress = 30;
            let additional = ((elapsed.as_millis() - 500) / 15).min(40) as u16; // Up to 40% more
            (
                base_progress + additional,
                "Removing",
                "Removing application files...",
                "Uninstalling",
            )
        }
        UpdateStage::UninstallCleaning => (
            80,
            "Cleaning",
            "Cleaning up dependencies...",
            "Uninstalling",
        ),
        UpdateStage::UninstallFinished => (
            100,
            "Complete",
            "Uninstall completed successfully! Closing...",
            "Uninstalling",
        ),
    };

    // Create modal content
    let progress_text = format!("{}% - {}", progress, stage_text);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(details, Style::default().fg(Color::Cyan))),
        Line::from(""),
        Line::from(progress_text),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            if app.is_uninstalling {
                "Uninstall in progress... Please wait for completion."
            } else {
                "Update in progress... Please wait for completion."
            },
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    // Create the modal block
    let title = format!("{} {}", modal_title, package_name);
    let modal_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .style(Style::default().bg(Color::Black));

    // Split modal area for content and progress bar
    let modal_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(content.len() as u16 + 1),
            Constraint::Length(3),
        ])
        .split(modal_block.inner(modal_area));

    // Render modal background
    f.render_widget(modal_block, modal_area);

    // Render content
    let content_paragraph = Paragraph::new(content)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(content_paragraph, modal_layout[0]);

    // Render progress bar
    let progress_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Green))
        .percent(progress)
        .label(format!("{}%", progress));
    f.render_widget(progress_gauge, modal_layout[1]);
}

/// Renders the uninstall confirmation modal
fn render_uninstall_confirmation_modal(f: &mut Frame, app: &App) {
    let area = f.area();

    // Create a centered modal area
    let modal_width = 50;
    let modal_height = 8;
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = ratatui::layout::Rect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    };

    // Clear the area behind the modal
    f.render_widget(Clear, modal_area);

    // Get package name
    let package_name = app
        .pending_uninstall_package
        .as_deref()
        .unwrap_or("Unknown Package");

    // Create modal content
    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Are you sure you want to uninstall '{}'?", package_name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This action cannot be undone.",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to confirm, ", Style::default().fg(Color::Gray)),
            Span::styled(
                "N",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to cancel", Style::default().fg(Color::Gray)),
        ]),
    ];

    // Create the modal block
    let modal_block = Block::default()
        .title("⚠️  Confirm Uninstall")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(Color::Black));

    // Render modal background
    f.render_widget(modal_block.clone(), modal_area);

    // Render content
    let content_paragraph = Paragraph::new(content)
        .block(modal_block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(content_paragraph, modal_area);
}
