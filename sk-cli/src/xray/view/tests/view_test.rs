use std::fs::File;
use std::io::BufReader;

use insta::assert_debug_snapshot;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use sk_store::ExportedTrace;

use super::*;
use crate::validation::tests::{
    annotated_trace,
    test_validation_store,
};
use crate::validation::{
    AnnotatedTrace,
    AnnotatedTraceEvent,
    ValidationStore,
};
use crate::xray::view::jump_list_state;

#[fixture]
fn test_app(test_validation_store: ValidationStore, mut annotated_trace: AnnotatedTrace) -> App {
    test_validation_store.validate_trace(&mut annotated_trace, false).unwrap();
    App {
        annotated_trace,
        event_list_state: ListState::default().with_selected(Some(0)),
        ..Default::default()
    }
}

#[fixture]
fn test_app_large(test_validation_store: ValidationStore) -> App {
    let trace_data_file = File::open("../testdata/large_trace.json").unwrap();
    let reader = BufReader::new(trace_data_file);
    let exported_trace: ExportedTrace = serde_json::from_reader(reader).unwrap();
    let annotated_trace = AnnotatedTrace::new_with_events(
        exported_trace
            .events()
            .iter()
            .cloned()
            .map(|e| AnnotatedTraceEvent::new(e))
            .collect(),
    );

    test_app(test_validation_store, annotated_trace)
}

#[rstest]
#[case::top(0, 0, false, 19, 19)]
#[case::top_mid_sel(10, 0, false, 19, 19)]
#[case::top_pin(0, 0, true, 19, 19)]
#[case::mid(20, 20, false, 39, 39)]
// the view height is 20, we are trying to jump to 39 but this would leave 17 empty spaces
// since there are only 42 elements in the list, so instead we set the offset to 22
#[case::mid_pin(20, 20, true, 39, 22)]
// when rendered, the "selected" index will become the last index in the list but for now we
// just naively add the viewport height to it, which is why the expected offset is 60 here
#[case::bottom(41, 41, false, 60, 41)]
#[case::bottom_pin(41, 41, true, 60, 22)]
fn test_jump_list_state_down(
    #[case] selected: usize,
    #[case] offset: usize,
    #[case] pin: bool,
    #[case] expected_selected: usize,
    #[case] expected_offset: usize,
) {
    let mut list_state = ListState::default().with_offset(offset).with_selected(Some(selected));
    jump_list_state(&mut list_state, JumpDir::Down, 42, 20, pin);
    assert_eq!(list_state.offset(), expected_offset);
    assert_eq!(list_state.selected().unwrap(), expected_selected);
}

#[rstest]
#[case::top(0, 0, 0, 0)]
#[case::top_mid_sel(10, 0, 0, 0)]
#[case::mid_to_top(10, 10, 0, 0)]
#[case::mid(23, 23, 4, 4)]
#[case::mid_mid_sel(25, 23, 23, 23)]
#[case::bottom(42, 42, 23, 23)]
fn test_jump_list_state_up(
    #[case] selected: usize,
    #[case] offset: usize,
    #[case] expected_offset: usize,
    #[case] expected_selected: usize,
) {
    let mut list_state = ListState::default().with_offset(offset).with_selected(Some(selected));
    jump_list_state(&mut list_state, JumpDir::Up, 42, 20, false);
    assert_eq!(list_state.offset(), expected_offset);
    assert_eq!(list_state.selected().unwrap(), expected_selected);
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

#[rstest]
#[case::top_1(0, 0)]
#[case::top_2(10, 0)]
#[case::bottom_1(80, 80)]
#[case::bottom_2(88, 80)]
#[case::bottom_3(88, 88)]
fn itest_render_large_event_list(mut test_app_large: App, #[case] selected: usize, #[case] offset: usize) {
    set_snapshot_suffix!("{selected}.{offset}");
    test_app_large.event_list_state.select(Some(selected));
    *test_app_large.event_list_state.offset_mut() = offset;
    let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
    let cf = term.draw(|frame| view(&mut test_app_large, frame)).unwrap();
    assert_debug_snapshot!(cf);
}

#[rstest]
#[case::top_1(1, 0)]
#[case::top_2(3, 0)]
#[case::bottom_1(80, 80)]
#[case::bottom_2(88, 80)]
#[case::bottom_3(88, 88)]
fn itest_render_large_event_list_event_selected(
    mut test_app_large: App,
    #[case] selected: usize,
    #[case] offset: usize,
) {
    set_snapshot_suffix!("{selected}.{offset}");
    test_app_large.mode = Mode::EventSelected;
    test_app_large.event_list_state.select(Some(selected));
    *test_app_large.event_list_state.offset_mut() = offset;
    let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
    let cf = term.draw(|frame| view(&mut test_app_large, frame)).unwrap();
    assert_debug_snapshot!(cf);
}
