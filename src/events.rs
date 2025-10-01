use crate::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

/// Handles keyboard events and updates application state accordingly
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<()> {
    if app.is_searching {
        handle_search_mode_keys(app, key)
    } else {
        handle_normal_mode_keys(app, key)
    }
}

/// Handles key events in normal navigation mode
fn handle_normal_mode_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        KeyCode::Left | KeyCode::Char('h') => app.move_left(),
        KeyCode::Right | KeyCode::Char('l') => app.move_right(),
        KeyCode::PageDown => app.page_down(),
        KeyCode::PageUp => app.page_up(),
        KeyCode::Home => app.first(),
        KeyCode::End => app.go_to_last(),
        KeyCode::Char('/') => app.start_search(),
        KeyCode::Char('i') => app.install_selected_package()?,
        KeyCode::Char('x') => app.uninstall_selected_package()?,
        KeyCode::Char('u') => app.update_selected_package()?,
        KeyCode::Char('r') => app.refresh_packages()?,
        _ => {}
    }
    Ok(())
}

/// Handles key events in search mode
fn handle_search_mode_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => app.end_search(),
        KeyCode::Enter => app.end_search(),
        KeyCode::Backspace => app.remove_search_char(),
        KeyCode::Down | KeyCode::Char('j')
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.next()
        }
        KeyCode::Up | KeyCode::Char('k')
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            app.previous()
        }
        KeyCode::Char(c) if c.is_ascii() && !c.is_control() => app.add_search_char(c),
        _ => {}
    }
    Ok(())
}
