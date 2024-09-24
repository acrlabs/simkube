use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::Model;

pub(super) fn view(_model: &Model, frame: &mut Frame) {
    let greeting = Paragraph::new("Hello Ratatui! (press 'q' to quit)").white().on_blue();
    frame.render_widget(greeting, frame.area());
}
