use ratatui::crossterm::event::{
    read,
    Event,
    KeyCode,
    KeyEventKind,
};

use super::{
    App,
    Message,
};

pub(super) fn handle_event(_app: &App) -> anyhow::Result<Message> {
    if let Event::Key(key) = read()? {
        if key.kind == KeyEventKind::Press {
            return Ok(match key.code {
                KeyCode::Char(' ') => Message::Select,
                KeyCode::Down | KeyCode::Char('j') => Message::Down,
                KeyCode::Esc => Message::Deselect,
                KeyCode::Up | KeyCode::Char('k') => Message::Up,
                KeyCode::Char('q') => Message::Quit,
                _ => Message::Unknown,
            });
        }
    }
    Ok(Message::Unknown)
}
