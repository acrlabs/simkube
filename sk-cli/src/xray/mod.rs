mod app;
mod event;
mod util;
mod view;

use ratatui::backend::Backend;
use ratatui::Terminal;
use sk_core::prelude::*;

use self::app::{
    App,
    Message,
};
use self::event::handle_event;
use self::view::view;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long_help = "location of the input trace file")]
    pub trace_path: String,
}

pub async fn cmd(args: &Args) -> EmptyResult {
    let app = App::new(&args.trace_path).await?;
    let term = ratatui::init();
    let res = run_loop(term, app);
    ratatui::restore();
    res
}

fn run_loop<B: Backend>(mut term: Terminal<B>, mut app: App) -> EmptyResult {
    let mut trace_changed = true;
    while app.running {
        if trace_changed {
            app.rebuild_annotated_trace();
        }
        term.draw(|frame| view(&mut app, frame))?;
        let msg = handle_event()?;
        trace_changed = app.update_state(msg);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
