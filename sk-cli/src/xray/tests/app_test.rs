use ratatui::widgets::ListState;

use super::*;

#[rstest]
fn test_app_update_quit() {
    let mut app = App { running: true, ..Default::default() };
    app.update(Message::Quit);
    assert!(!app.running);
}

#[rstest]
#[case(Message::Deselect, Mode::ObjectSelected, Mode::EventSelected)]
#[case(Message::Deselect, Mode::EventSelected, Mode::RootView)]
#[case(Message::Deselect, Mode::RootView, Mode::RootView)]
#[case(Message::Select, Mode::RootView, Mode::EventSelected)]
#[case(Message::Select, Mode::EventSelected, Mode::ObjectSelected)]
#[case(Message::Select, Mode::ObjectSelected, Mode::ObjectSelected)]
fn test_app_update_selection(#[case] msg: Message, #[case] mode: Mode, #[case] new_mode: Mode) {
    let mut app = App { mode, ..Default::default() };
    app.update(msg);
    assert_eq!(app.mode, new_mode);
}

#[rstest]
#[case(Mode::RootView, Message::Down)]
#[case(Mode::EventSelected, Message::Down)]
#[case(Mode::ObjectSelected, Message::Down)]
#[case(Mode::RootView, Message::Up)]
#[case(Mode::EventSelected, Message::Up)]
#[case(Mode::ObjectSelected, Message::Up)]
fn test_app_update_nav(#[case] mode: Mode, #[case] msg: Message) {
    let (start, end) = match msg {
        Message::Down => (0, 1),
        Message::Up => (1, 0),
        _ => panic!("shouldn't occur"),
    };

    let mut app = App {
        mode,
        event_list_state: ListState::default().with_selected(Some(start)),
        object_list_state: ListState::default().with_selected(Some(start)),
        object_contents_list_state: ListState::default().with_selected(Some(start)),
        ..Default::default()
    };
    app.update(msg);
    match mode {
        Mode::RootView => assert_eq!(app.event_list_state.selected(), Some(end)),
        Mode::EventSelected => assert_eq!(app.object_list_state.selected(), Some(end)),
        Mode::ObjectSelected => assert_eq!(app.object_contents_list_state.selected(), Some(end)),
    }
}
