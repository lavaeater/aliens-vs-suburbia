mod app;
mod input;
mod ui;

use std::io;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use input::Action;

pub fn run(file_path: Option<String>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::new(file_path);

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                // Compute visible tile area for scrolling: terminal size minus borders/status
                let size = terminal.size()?;
                let cell_w = 2usize;
                let canvas_h = (size.height as usize).saturating_sub(6); // borders + status
                let canvas_w = (size.width as usize).saturating_sub(2);  // borders
                let viewport_cols = canvas_w / cell_w;
                let viewport_rows = canvas_h;

                let action = input::handle_key(&mut app, key, (viewport_cols, viewport_rows));

                // __quit__ sentinel from confirm-quit prompt
                if matches!(app.status_msg.as_deref(), Some("__quit__")) {
                    break;
                }

                if matches!(action, Action::Quit) {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
