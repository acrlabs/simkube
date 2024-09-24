#[derive(Eq, PartialEq)]
pub(super) enum ApplicationState {
    Running,
    Done,
}

pub(super) struct Model {
    pub(super) app_state: ApplicationState,
}

impl Model {
    pub(super) fn new() -> Model {
        Model { app_state: ApplicationState::Running }
    }
}
