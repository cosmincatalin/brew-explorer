mod app;
mod entities;
mod events;
mod helpers;
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
    io, thread,
    time::{Duration, Instant},
};
use ui::{render_loading_screen, render_ui};

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
        // First, run brew update to fetch latest package information
        let _ = helpers::brew_update();

        // Then create repository and app
        let repository = HomebrewRepository::new();
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
            && key.kind == KeyEventKind::Press
            && key.code == event::KeyCode::Char('q')
        {
            // Cleanup and exit
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
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
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| render_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key_event(app, key)?;
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

            // Update mock update progress
            app.update_mock_progress();

            last_tick = Instant::now();
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
