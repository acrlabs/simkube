mod event;
mod model;
mod update;
mod view;

use ratatui::backend::Backend;
use ratatui::Terminal;
use sk_core::prelude::*;

use self::event::handle_event;
use self::model::{
    ApplicationState,
    Model,
};
use self::update::{
    update,
    Message,
};
use self::view::view;

pub fn cmd() -> EmptyResult {
    let model = Model::new();
    let term = ratatui::init();
    let res = run_loop(term, model);
    ratatui::restore();
    res
}

fn run_loop<B: Backend>(mut term: Terminal<B>, mut model: Model) -> EmptyResult {
    while model.app_state != ApplicationState::Done {
        term.draw(|frame| view(&model, frame))?;

        let msg: Message = handle_event(&model)?;

        update(&mut model, msg);
    }
    Ok(())
}
