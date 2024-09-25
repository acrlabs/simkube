use super::app::{
    App,
    Mode,
};

pub(super) enum Message {
    Deselect,
    Down,
    Quit,
    Select,
    Unknown,
    Up,
}

pub(super) fn update(app: &mut App, msg: Message) {
    match msg {
        Message::Deselect => match app.mode {
            Mode::ObjectSelected => {
                app.mode = Mode::EventSelected;
                app.object_contents_list_state.select(None);
            },
            Mode::EventSelected => app.mode = Mode::RootView,
            _ => (),
        },
        Message::Down => match app.mode {
            Mode::ObjectSelected => app.object_contents_list_state.select_next(),
            Mode::EventSelected => app.object_list_state.select_next(),
            Mode::RootView => app.event_list_state.select_next(),
        },
        Message::Quit => app.running = false,
        Message::Select => match app.mode {
            Mode::EventSelected => {
                app.mode = Mode::ObjectSelected;
                app.object_contents_list_state.select(Some(0));
            },
            Mode::RootView => {
                app.mode = Mode::EventSelected;
                app.object_list_state.select(Some(0));
            },
            _ => (),
        },
        Message::Unknown => (),
        Message::Up => match app.mode {
            Mode::ObjectSelected => app.object_contents_list_state.select_previous(),
            Mode::EventSelected => app.object_list_state.select_previous(),
            Mode::RootView => app.event_list_state.select_previous(),
        },
    }
}
