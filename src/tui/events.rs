use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::time::Duration;

use super::app::App;

pub fn handle_events(app: &mut App) -> std::io::Result<bool> {
    if !event::poll(Duration::from_millis(200))? {
        return Ok(false);
    }
    let Event::Key(key) = event::read()? else {
        return Ok(false);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }
    dispatch(app, key);
    Ok(true)
}

fn dispatch(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Char('h') {
        app.show_help = !app.show_help;
        return;
    }
    if app.show_help {
        if key.code == KeyCode::Esc {
            app.show_help = false;
        }
        return;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc if app.show_detail => app.show_detail = false,
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Enter => app.toggle_detail(),
        KeyCode::Tab => app.toggle_focus(),
        KeyCode::Char('t') => app.cycle_theme(),
        _ => {}
    }
}
