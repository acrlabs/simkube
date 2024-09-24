use ratatui::crossterm::event::{
    read,
    Event,
    KeyCode,
    KeyEventKind,
};

use super::{
    Message,
    Model,
};

pub(super) fn handle_event(_model: &Model) -> anyhow::Result<Message> {
    if let Event::Key(key) = read()? {
        if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
            return Ok(Message::Quit);
        }
    }
    Ok(Message::Unknown)
}
