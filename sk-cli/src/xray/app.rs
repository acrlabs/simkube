use ratatui::widgets::ListState;

use crate::validation::{
    AnnotatedTrace,
    ValidationStore,
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

    pub(super) fn update(&mut self, msg: Message) {
        match msg {
            Message::Deselect => match self.mode {
                Mode::ObjectSelected => {
                    self.mode = Mode::EventSelected;
                    self.object_contents_list_state.select(None);
                },
                Mode::EventSelected => self.mode = Mode::RootView,
                _ => (),
            },
            Message::Down => match self.mode {
                Mode::ObjectSelected => self.object_contents_list_state.select_next(),
                Mode::EventSelected => self.object_list_state.select_next(),
                Mode::RootView => self.event_list_state.select_next(),
            },
            Message::Quit => self.running = false,
            Message::Select => match self.mode {
                Mode::EventSelected => {
                    self.mode = Mode::ObjectSelected;
                    self.object_contents_list_state.select(Some(0));
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
    }
}
