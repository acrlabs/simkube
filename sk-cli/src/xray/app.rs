use ratatui::widgets::ListState;

use crate::validation::{
    AnnotatedTrace,
    ValidationStore,
};

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) enum Mode {
    #[default]
    RootView,
    EventSelected,
    ObjectSelected,
}

#[derive(Default)]
pub(super) struct App {
    pub(super) running: bool,
    pub(super) mode: Mode,

    pub(super) trace: AnnotatedTrace,
    #[allow(dead_code)]
    pub(super) validation_store: ValidationStore,

    pub(super) event_list_state: ListState,
    pub(super) object_list_state: ListState,
    pub(super) object_contents_list_state: ListState,
}

impl App {
    pub(super) async fn new(trace_path: &str) -> anyhow::Result<App> {
        Ok(App {
            running: true,
            trace: AnnotatedTrace::new(trace_path).await?,
            event_list_state: ListState::default().with_selected(Some(0)),

            ..Default::default()
        })
    }
}
