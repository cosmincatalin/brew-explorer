use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::time::Duration;

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
}

/// Renders the package list on the left panel
fn render_package_list(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let available_width = area.width.saturating_sub(4) as usize; // Account for borders
    let items = app.get_display_items();

    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, package)| {
            let content = if Some(i) == app.list_state.selected() {
                // Apply horizontal scrolling to the selected item
                apply_horizontal_scroll(&package.name, available_width, app)
            } else {
                package.name.clone()
            };

            let style = if package.is_installed() {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

    let title = if app.is_searching {
        format!("Packages (Search: {})", app.search_query)
    } else {
        "Packages".to_string()
    };

    let items_list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(items_list, area, &mut app.list_state);
}

/// Applies horizontal scrolling to a package name
fn apply_horizontal_scroll(name: &str, available_width: usize, app: &App) -> String {
    let name_len = name.len();

    if name_len > available_width && app.last_interaction.elapsed() > Duration::from_secs(3) {
        let start = app.scroll_offset % name_len.max(1);
        let end = (start + available_width).min(name_len);

        if start < name_len {
            if end <= name_len {
                name[start..end].to_string()
            } else {
                // Wrap around
                let first_part = &name[start..];
                let second_part = &name[..end - name_len];
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
        .wrap(Wrap { trim: true });

    f.render_widget(details_paragraph, area);

    // Render help text at the bottom
    render_help_text(f, area);
}

/// Creates the detailed text for a package
fn create_package_details_text(package: &crate::models::PackageInfo) -> Text {
    let installed_status = package.installation_status();
    let status_colour = if package.is_installed() {
        Color::Green
    } else {
        Color::Red
    };

    Text::from(vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&package.name),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Description: ",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(Span::raw(&package.description)),
        Line::from(""),
        Line::from(vec![
            Span::styled("Homepage: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&package.homepage, Style::default().fg(Color::Blue)),
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
        Line::from(vec![
            Span::styled(
                "Installed Version: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(installed_status, Style::default().fg(status_colour)),
        ]),
        Line::from(""),
        Line::from(""),
        create_action_hints(package),
    ])
}

/// Creates action hints based on package state
fn create_action_hints(package: &crate::models::PackageInfo) -> Line {
    if package.is_installed() {
        if package.has_update_available() {
            Line::from(vec![
                Span::styled("Actions: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("u", Style::default().fg(Color::Yellow)),
                Span::raw("pdate, "),
                Span::styled("x", Style::default().fg(Color::Red)),
                Span::raw(" uninstall"),
            ])
        } else {
            Line::from(vec![
                Span::styled("Actions: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("x", Style::default().fg(Color::Red)),
                Span::raw(" uninstall"),
            ])
        }
    } else {
        Line::from(vec![
            Span::styled("Actions: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("i", Style::default().fg(Color::Green)),
            Span::raw("nstall"),
        ])
    }
}

/// Renders help text at the bottom of the details panel
fn render_help_text(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = "Navigate: ↑/↓ or j/k | Search: / | Actions: i/u/x | Quit: q";
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
    let status_text = if let Some(message) = app.get_current_status() {
        message
    } else {
        "Navigate: ↑/↓ or j/k | Search: / | Actions: i/u/x | Quit: q".to_string()
    };

    let status_paragraph = Paragraph::new(status_text)
        .style(Style::default().bg(Color::Blue).fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(status_paragraph, area);
}
