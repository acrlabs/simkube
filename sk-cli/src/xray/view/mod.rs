mod helpers;

use std::iter::{
    once,
    repeat,
};

use ratatui::prelude::*;
use ratatui::widgets::{
    Block,
    Borders,
    Clear,
    List,
    Padding,
    Paragraph,
};

use self::helpers::*;
use super::app::{
    App,
    Mode,
};

const MIN_TWO_PANEL_WIDTH: u16 = 120;

pub(super) fn view(app: &mut App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(100), Constraint::Min(5)])
        .split(frame.area());
    let (top, bottom) = (layout[0], layout[1]);

    let events_border = Block::bordered().title(app.annotated_trace.path());
    let object_border = Block::bordered();

    if top.width > MIN_TWO_PANEL_WIDTH {
        let lr_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(top);
        let (left, right) = (lr_layout[0], lr_layout[1]);

        render_event_list(app, frame, events_border.inner(left));
        if app.mode == Mode::EventSelected || app.mode == Mode::ObjectSelected {
            render_object(app, frame, object_border.inner(right));
        }

        frame.render_widget(events_border, left);
        frame.render_widget(object_border, right);
    } else {
        render_event_list(app, frame, events_border.inner(top));
        frame.render_widget(events_border, top);

        if app.mode == Mode::ObjectSelected {
            let popup = Rect {
                x: top.width / 10,
                y: top.height / 10,
                width: 4 * top.width / 5,
                height: 4 * top.height / 5,
            };
            frame.render_widget(Clear, popup);
            render_object(app, frame, object_border.inner(popup));
            frame.render_widget(object_border, popup);
        }
    }

    let greeting2 = Paragraph::new("Hello SimKube!\nUse arrows to navigate, space to select, 'q' to quit.")
        .white()
        .block(Block::new().borders(Borders::ALL));
    frame.render_widget(greeting2, bottom);
}

fn render_event_list(app: &mut App, frame: &mut Frame, layout: Rect) {
    // Here's some sortof obnoxious code; we'd like to have the event list "expand" so that you can
    // see the applied and deleted objects for that particular event.  The way we do this is split
    // our layout into three sublayouts; the first includes all the events up to the selected one,
    // then we nest in one level and display the applied and deleted objects, then we unnest and
    // display the rest of the events
    let num_events = app.annotated_trace.len();
    let start_ts = app.annotated_trace.start_ts().unwrap_or(0);

    // Add one so the selected event is included on top
    let (sel_index_inclusive, sel_event) = match app.mode {
        Mode::EventSelected | Mode::ObjectSelected => {
            let sel_index = app.selected_event_index();
            (sel_index + 1, app.annotated_trace.get_event(sel_index))
        },
        _ => (num_events, None),
    };

    let event_spans = app.annotated_trace.iter().map(|event| make_event_spans(event, start_ts));
    let mut top_entries = format_list_entries(event_spans, layout.width as usize);
    let bottom_entries = top_entries.split_off(sel_index_inclusive);

    let obj_spans = sel_event.map_or(vec![], |evt| {
        let mut sublist_items = evt
            .data
            .applied_objs
            .iter()
            .zip(repeat('+'))
            .chain(evt.data.deleted_objs.iter().zip(repeat('-')))
            .enumerate()
            .map(|(i, (obj, op))| make_object_spans(i, obj, op, evt))
            .peekable();
        if sublist_items.peek().is_none() {
            format_list_entries(once((Span::default(), Span::default())), layout.width as usize)
        } else {
            format_list_entries(sublist_items, layout.width as usize)
        }
    });

    let nested_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            // We know how many lines we have; use max constraints here so the lists are next to
            // each other.  The last one can be min(0) and take up the rest of the space
            Constraint::Max(top_entries.len() as u16),
            Constraint::Max(obj_spans.len() as u16),
            Constraint::Min(0),
        ])
        .split(layout);

    let list_part_one = List::new(top_entries)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");
    let sublist = List::new(obj_spans)
        .highlight_style(Style::new().bg(Color::Blue))
        .highlight_symbol("++ ");
    let list_part_two = List::new(bottom_entries).block(Block::new().padding(Padding::left(LIST_PADDING as u16)));

    frame.render_stateful_widget(list_part_one, nested_layout[0], &mut app.event_list_state);
    frame.render_stateful_widget(sublist, nested_layout[1], &mut app.object_list_state);
    frame.render_widget(list_part_two, nested_layout[2]);
}

fn render_object(app: &mut App, frame: &mut Frame, layout: Rect) {
    let event_idx = app.event_list_state.selected().unwrap();
    let obj_idx = app.object_list_state.selected().unwrap();

    let Some(obj) = app.annotated_trace.get_object(event_idx, obj_idx) else {
        return;
    };
    let obj_str = serde_json::to_string_pretty(obj).unwrap();
    let contents = List::new(obj_str.split('\n')).highlight_style(Style::new().bg(Color::Blue));
    frame.render_stateful_widget(contents, layout, &mut app.object_contents_list_state);
}
