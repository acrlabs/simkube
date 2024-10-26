use insta::assert_debug_snapshot;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;
use ratatui::widgets::ListState;

use super::*;
use crate::validation::tests::{
    annotated_trace,
    test_validation_store,
};
use crate::validation::{
    AnnotatedTrace,
    ValidationStore,
};

#[fixture]
fn test_app(mut test_validation_store: ValidationStore, mut annotated_trace: AnnotatedTrace) -> App {
    test_validation_store.validate_trace(&mut annotated_trace);
    App {
        annotated_trace,
        event_list_state: ListState::default().with_selected(Some(0)),
        ..Default::default()
    }
}

#[rstest]
#[case::first(0)]
#[case::last(3)]
fn itest_render_event_list(mut test_app: App, #[case] index: usize) {
    set_snapshot_suffix!("{index}");
    test_app.event_list_state.select(Some(index));
    let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
    let cf = term.draw(|frame| view(&mut test_app, frame)).unwrap();
    assert_debug_snapshot!(cf);
}

#[rstest]
#[case::first(0)]
#[case::middle(2)]
#[case::last(3)]
fn itest_render_event_list_event_selected(mut test_app: App, #[case] index: usize) {
    set_snapshot_suffix!("{index}");
    test_app.mode = Mode::EventSelected;
    test_app.event_list_state.select(Some(index));
    test_app.object_list_state.select(Some(0));
    let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
    let cf = term.draw(|frame| view(&mut test_app, frame)).unwrap();
    assert_debug_snapshot!(cf);
}
