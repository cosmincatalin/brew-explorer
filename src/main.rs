mod app;
mod events;
mod models;
mod repository;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use events::handle_key_event;
use ratatui::{Terminal, backend::CrosstermBackend, prelude::Backend};
use repository::HomebrewRepository;
use std::{
    io,
    time::{Duration, Instant},
};
use ui::render_ui;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app with repository
    let repository = Box::new(HomebrewRepository::new());
    let mut app = App::new(repository)?;
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    let mut last_refresh = Instant::now();

    loop {
        terminal.draw(|f| render_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(app, key)?;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // Update scroll offset for auto-scrolling
            let size = terminal.size()?;
            let rect = ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: size.width,
                height: size.height,
            };
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(40),
                    ratatui::layout::Constraint::Percentage(60),
                ])
                .split(rect);

            let available_width = chunks[0].width.saturating_sub(4) as usize; // Account for borders
            app.update_scroll(available_width);
            
            // Update repository status to get background caching messages
            app.update_repository_status();
            
            // Update package details from background loading
            app.update_package_details();
            
            last_tick = Instant::now();
        }
        
        // Refresh package list every 2 seconds to pick up cached packages
        if last_refresh.elapsed() >= Duration::from_secs(2) {
            if let Err(_) = app.refresh_package_list() {
                // Ignore refresh errors to avoid crashing the app
            }
            last_refresh = Instant::now();
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
