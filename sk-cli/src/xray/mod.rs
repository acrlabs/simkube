mod app;
mod event;
mod update;
mod util;
mod view;

use ratatui::backend::Backend;
use ratatui::Terminal;
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::prelude::*;
use sk_store::TraceStore;

use self::app::App;
use self::event::handle_event;
use self::update::{
    update,
    Message,
};
use self::view::view;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long_help = "location of the input trace file")]
    pub trace_path: String,
}

pub async fn cmd(args: &Args) -> EmptyResult {
    let object_store = SkObjectStore::new(&args.trace_path)?;
    let trace_data = object_store.get().await?.to_vec();
    let store = TraceStore::import(trace_data, &None)?;

    let app = App::new(&args.trace_path, store);
    let term = ratatui::init();
    let res = run_loop(term, app);
    ratatui::restore();
    res
}

fn run_loop<B: Backend>(mut term: Terminal<B>, mut app: App) -> EmptyResult {
    while app.running {
        term.draw(|frame| view(&mut app, frame))?;
        let msg: Message = handle_event(&app)?;
        update(&mut app, msg);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
