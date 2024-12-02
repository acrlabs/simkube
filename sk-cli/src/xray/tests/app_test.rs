use ratatui::widgets::ListState;
use sk_store::TraceEvent;

use super::*;
use crate::validation::{
    AnnotatedTrace,
    AnnotatedTraceEvent,
};

#[rstest]
fn test_app_update_quit() {
    let mut app = App { running: true, ..Default::default() };
    app.update_state(Message::Quit);
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
    let annotated_trace = AnnotatedTrace::new_with_events(vec![AnnotatedTraceEvent::new(TraceEvent {
        ts: 0,
        applied_objs: vec![test_deployment("depl1")],
        ..Default::default()
    })]);
    let mut app = App {
        mode,
        annotated_trace,
        event_list_state: ListState::default().with_selected(Some(0)),
        ..Default::default()
    };
    app.update_state(msg);
    assert_eq!(app.mode, new_mode);
}

#[rstest]
fn test_app_update_select_event_no_objects() {
    let mut app = App {
        mode: Mode::EventSelected,
        event_list_state: ListState::default().with_selected(Some(0)),
        ..Default::default()
    };
    app.update_state(Message::Select);
    assert_eq!(app.mode, Mode::EventSelected);
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
    app.update_state(msg);
    match mode {
        Mode::RootView => assert_eq!(app.event_list_state.selected(), Some(end)),
        Mode::EventSelected => assert_eq!(app.object_list_state.selected(), Some(end)),
        Mode::ObjectSelected => assert_eq!(app.object_contents_list_state.selected(), Some(end)),
    }
}
