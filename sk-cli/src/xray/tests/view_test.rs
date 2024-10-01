use insta::assert_debug_snapshot;
use kube::core::{
    ApiResource,
    DynamicObject,
    GroupVersionKind,
};
use ratatui::backend::TestBackend;
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use sk_store::TraceEvent;

use super::*;

#[fixture]
fn test_app() -> App {
    App {
        events: vec![
            TraceEvent { ts: 0, ..Default::default() },
            TraceEvent {
                ts: 1,
                applied_objs: vec![DynamicObject::new(
                    "test_depl1",
                    &ApiResource::from_gvk(&GroupVersionKind::gvk("core".into(), "v1".into(), "deployment".into())),
                )
                .within("test_namespace")],
                deleted_objs: vec![],
            },
            TraceEvent {
                ts: 2,
                applied_objs: vec![
                    DynamicObject::new(
                        "test_depl1",
                        &ApiResource::from_gvk(&GroupVersionKind::gvk("core".into(), "v1".into(), "deployment".into())),
                    )
                    .within("test_namespace"),
                    DynamicObject::new(
                        "test_depl2",
                        &ApiResource::from_gvk(&GroupVersionKind::gvk("core".into(), "v1".into(), "deployment".into())),
                    )
                    .within("test_namespace"),
                ],
                deleted_objs: vec![],
            },
            TraceEvent {
                ts: 3,
                applied_objs: vec![],
                deleted_objs: vec![DynamicObject::new(
                    "test_depl1",
                    &ApiResource::from_gvk(&GroupVersionKind::gvk("core".into(), "v1".into(), "deployment".into())),
                )
                .within("test_namespace")],
            },
        ],
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
