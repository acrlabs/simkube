use ratatui::widgets::ListState;

use crate::validation::{
    AnnotatedTrace,
    VALIDATORS,
};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub(super) enum Mode {
    #[default]
    RootView,
    EventSelected,
    ObjectSelected,
}

pub(super) enum Message {
    Deselect,
    Down,
    Quit,
    Select,
    Unknown,
    Up,
}

#[derive(Default)]
pub(super) struct App {
    pub(super) running: bool,
    pub(super) mode: Mode,

    pub(super) annotated_trace: AnnotatedTrace,

    pub(super) event_list_state: ListState,
    pub(super) object_list_state: ListState,
    pub(super) object_contents_list_state: ListState,
}

impl App {
    pub(super) async fn new(trace_path: &str) -> anyhow::Result<App> {
        Ok(App {
            running: true,
            annotated_trace: AnnotatedTrace::new(trace_path).await?,
            event_list_state: ListState::default().with_selected(Some(0)),

            ..Default::default()
        })
    }

    pub(super) fn rebuild_annotated_trace(&mut self) {
        VALIDATORS
            .validate_trace(&mut self.annotated_trace, false)
            .expect("validation failed");
    }

    pub(super) fn update_state(&mut self, msg: Message) -> bool {
        match msg {
            Message::Deselect => match self.mode {
                Mode::ObjectSelected => {
                    self.mode = Mode::EventSelected;
                    self.object_contents_list_state.select(None);
                },
                Mode::EventSelected => self.mode = Mode::RootView,
                Mode::RootView => self.running = false,
            },
            Message::Down => match self.mode {
                Mode::ObjectSelected => self.object_contents_list_state.select_next(),
                Mode::EventSelected => self.object_list_state.select_next(),
                Mode::RootView => self.event_list_state.select_next(),
            },
            Message::Quit => self.running = false,
            Message::Select => match self.mode {
                Mode::EventSelected => {
                    let i = self.selected_event_index();
                    if !self.annotated_trace.is_empty_at(i) {
                        self.mode = Mode::ObjectSelected;
                        self.object_contents_list_state.select(Some(0));
                    }
                },
                Mode::RootView => {
                    self.mode = Mode::EventSelected;
                    self.object_list_state.select(Some(0));
                },
                _ => (),
            },
            Message::Unknown => (),
            Message::Up => match self.mode {
                Mode::ObjectSelected => self.object_contents_list_state.select_previous(),
                Mode::EventSelected => self.object_list_state.select_previous(),
                Mode::RootView => self.event_list_state.select_previous(),
            },
        }

        false
    }

    pub(super) fn selected_event_index(&self) -> usize {
        self.event_list_state.selected().unwrap() // there should always be a selected event
    }
}
