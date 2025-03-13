use ratatui::crossterm::event::{read, Event, KeyCode, KeyEventKind, KeyModifiers};

use super::Message;

const NO_MOD: KeyModifiers = KeyModifiers::empty();

pub(super) fn handle_event() -> anyhow::Result<Message> {
    if let Event::Key(key) = read()? {
        if key.kind == KeyEventKind::Press {
            return Ok(match (key.code, key.modifiers) {
                // navigation
                (KeyCode::Up, NO_MOD) | (KeyCode::Char('k'), NO_MOD) => Message::Up,
                (KeyCode::Down, NO_MOD) | (KeyCode::Char('j'), NO_MOD) => Message::Down,
                (KeyCode::PageUp, NO_MOD) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => Message::PageUp,
                (KeyCode::PageDown, NO_MOD) | (KeyCode::Char('f'), KeyModifiers::CONTROL) => Message::PageDown,

                // selection
                (KeyCode::Char(' '), NO_MOD) => Message::Select,
                (KeyCode::Esc, NO_MOD) => Message::Deselect,

                // app controls
                (KeyCode::Char('q'), NO_MOD) => Message::Quit,
                _ => Message::Unknown,
            });
        }
    }
    Ok(Message::Unknown)
}
