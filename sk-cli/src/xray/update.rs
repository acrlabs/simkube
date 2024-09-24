use super::{
    ApplicationState,
    Model,
};

pub(super) enum Message {
    Quit,
    Unknown,
}

pub(super) fn update(model: &mut Model, msg: Message) {
    match msg {
        Message::Quit => model.app_state = ApplicationState::Done,
        Message::Unknown => (),
    }
}
