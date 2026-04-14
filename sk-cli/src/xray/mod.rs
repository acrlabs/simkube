mod app;
mod event;
mod view;

use ratatui::Terminal;
use ratatui::backend::Backend;
use sk_core::prelude::*;
use sk_store::ExportedTrace;

use self::app::{
    App,
    Message,
};
use self::event::handle_event;
use self::view::view;
use crate::validation::VALIDATORS;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long_help = "location of the input trace file")]
    pub trace_path: String,
}

pub async fn cmd(args: &Args) -> EmptyResult {
    let trace = ExportedTrace::from_path(&args.trace_path).await?;
    let event_annotations = VALIDATORS.lock().unwrap().validate_trace(&trace)?;
    let app = App::new(&args.trace_path, trace, event_annotations);
    let term = ratatui::init();
    let res = run_loop(term, app);
    ratatui::restore();
    res
}

fn run_loop<B: Backend>(mut term: Terminal<B>, mut app: App) -> EmptyResult {
    while app.running {
        term.draw(|frame| view(&mut app, frame))?;
        let msg = handle_event()?;
        app.update_state(msg);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
