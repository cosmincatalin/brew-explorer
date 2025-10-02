mod app;
mod events;
mod models;
mod repository;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, poll},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use events::handle_key_event;
use ratatui::{Terminal, backend::CrosstermBackend, prelude::Backend};
use repository::HomebrewRepository;
use std::{
    io,
    thread,
    time::{Duration, Instant},
};
use ui::{render_ui, render_loading_screen};

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Show loading screen while initializing
    let start_time = Instant::now();
    let mut loading_dots = 0;
    let mut last_dot_update = Instant::now();
    
    // Create repository and app in a separate thread to show real loading progress
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let repository = Box::new(HomebrewRepository::new());
        let app = App::new(repository);
        tx.send(app).unwrap();
    });
    
    // Show loading screen until app is ready (real loading time)
    let mut app = loop {
        // Update loading animation every 200ms for smoother animation
        if last_dot_update.elapsed() >= Duration::from_millis(200) {
            loading_dots = (loading_dots + 1) % 4;
            last_dot_update = Instant::now();
        }
        
        // Render loading screen
        terminal.draw(|f| render_loading_screen(f, loading_dots, start_time.elapsed()))?;
        
        // Check if app is ready
        if let Ok(app_result) = rx.try_recv() {
            break app_result?;
        }
        
        // Handle any key events during loading (allow quit)
        if poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press && key.code == crossterm::event::KeyCode::Char('q') {
                    // Cleanup and exit
                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
                    terminal.show_cursor()?;
                    return Ok(());
                }
        
        thread::sleep(Duration::from_millis(50));
    };
    
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
    let tick_rate = Duration::from_millis(250); // Slower tick rate for better performance
    let mut last_tick = Instant::now();
    let mut last_refresh = Instant::now();
    let mut last_layout_calc = Instant::now();
    let mut last_status_update = Instant::now();
    let mut cached_terminal_size = terminal.size()?;

    loop {
        terminal.draw(|f| render_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press {
                    handle_key_event(app, key)?;
                }

        if last_tick.elapsed() >= tick_rate {
            // Only update scroll offset if we're actually updating
            if app.is_updating || app.update_package_details_if_needed() {
                // Only recalculate layout if terminal size changed or we haven't calculated in a while
                let current_size = terminal.size()?;
                if current_size != cached_terminal_size || last_layout_calc.elapsed() >= Duration::from_secs(1) {
                    cached_terminal_size = current_size;
                    let rect = ratatui::layout::Rect {
                        x: 0,
                        y: 0,
                        width: current_size.width,
                        height: current_size.height,
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
                    last_layout_calc = Instant::now();
                }
            }
            
            // Update repository status less frequently
            if last_status_update.elapsed() >= Duration::from_secs(1) {
                app.update_repository_status();
                last_status_update = Instant::now();
            }
            
            // Update mock update progress
            app.update_mock_progress();
            
            last_tick = Instant::now();
        }
        
        // Refresh package list only when needed - check every 60 seconds but only refresh if not actively using the app
        if last_refresh.elapsed() >= Duration::from_secs(60) && app.last_interaction.elapsed() >= Duration::from_secs(10) {
            if app.refresh_package_list().is_err() {
                // Ignore refresh errors to avoid crashing the app
            }
            last_refresh = Instant::now();
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
