use ratatui::widgets::ListState;
use sk_store::{
    TraceEvent,
    TraceStorable,
    TraceStore,
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

    pub(super) base_trace: TraceStore,
    pub(super) trace_path: String,
    pub(super) events: Vec<TraceEvent>,

    pub(super) event_list_state: ListState,
    pub(super) object_list_state: ListState,
    pub(super) object_contents_list_state: ListState,
}

impl App {
    pub(super) fn new(trace_path: &str, base_trace: TraceStore) -> App {
        let events = base_trace.iter().map(|(evt, _)| evt).cloned().collect();
        App {
            running: true,

            base_trace,
            trace_path: trace_path.into(),
            events,

            event_list_state: ListState::default().with_selected(Some(0)),

            ..Default::default()
        }
    }
}
