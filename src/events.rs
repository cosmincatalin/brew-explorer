use crate::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

/// Handles keyboard events and updates application state accordingly
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<()> {
    // Check if we're in a modal state first
    if app.modal_state != crate::app::ModalState::None {
        handle_modal_keys(app, key)
    } else if app.is_searching {
        handle_search_mode_keys(app, key)
    } else {
        handle_normal_mode_keys(app, key)
    }
}

/// Handles key events when a modal is open
fn handle_modal_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match app.modal_state {
        crate::app::ModalState::UpdateProgress => {
            // During update progress, no keys are allowed - user must wait for completion
            // The modal will automatically close when the update finishes
            match key.code {
                KeyCode::Char('q') => {
                    // Allow quitting the entire application even during update
                    app.quit();
                }
                _ => {
                    // Ignore all other keys during update
                }
            }
        }
        crate::app::ModalState::UninstallConfirmation => {
            // Handle uninstall confirmation dialog
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    app.confirm_uninstall();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.cancel_uninstall();
                }
                KeyCode::Char('q') => {
                    // Allow quitting the entire application
                    app.quit();
                }
                _ => {
                    // Ignore other keys
                }
            }
        }
        crate::app::ModalState::None => {
            // This shouldn't happen, but handle gracefully
        }
    }
    Ok(())
}

/// Handles key events in normal navigation mode
fn handle_normal_mode_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Left => app.move_left(),
        KeyCode::Right => app.move_right(),
        KeyCode::PageDown => app.page_down(),
        KeyCode::PageUp => app.page_up(),
        KeyCode::Home => app.first(),
        KeyCode::End => app.go_to_last(),
        KeyCode::Char('/') => app.start_search(),
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
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Left => app.move_left(),
        KeyCode::Right => app.move_right(),
        KeyCode::PageDown => app.page_down(),
        KeyCode::PageUp => app.page_up(),
        KeyCode::Home => app.first(),
        KeyCode::End => app.go_to_last(),
        KeyCode::Char(c) if c.is_ascii() && !c.is_control() => app.add_search_char(c),
        _ => {}
    }
    Ok(())
}
