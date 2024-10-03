use std::iter::repeat;

use chrono::TimeDelta;
use ratatui::prelude::*;
use ratatui::widgets::{
    Block,
    Borders,
    Clear,
    List,
    Padding,
    Paragraph,
};
use sk_core::k8s::KubeResourceExt;
use sk_store::TraceStorable;

use super::app::{
    App,
    Mode,
};
use super::util::format_duration;

pub(super) fn view(app: &mut App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(100), Constraint::Min(5)])
        .split(frame.area());
    let (top, bottom) = (layout[0], layout[1]);

    let events_border = Block::bordered().title(app.trace.path.clone());
    let object_border = Block::bordered();

    if top.width > 120 {
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
    let num_events = app.trace.events.len();
    let start_ts = app.trace.base.start_ts().unwrap_or(0);

    // Add one so the selected event is included on top
    let (sel_index_inclusive, sel_event) = match app.mode {
        Mode::EventSelected | Mode::ObjectSelected => {
            let sel_index = app.event_list_state.selected().unwrap();
            (sel_index + 1, Some(&app.trace.events[sel_index]))
        },
        _ => (num_events, None),
    };

    let mut root_items_1 = Vec::with_capacity(sel_index_inclusive);
    let mut root_items_2 = Vec::with_capacity(num_events - sel_index_inclusive);

    for (i, evt) in app.trace.events.iter().enumerate() {
        let d = TimeDelta::new(evt.data.ts - start_ts, 0).unwrap();
        let d_str = format!(
            "{} ({} applied/{} deleted)",
            format_duration(d),
            evt.data.applied_objs.len(),
            evt.data.deleted_objs.len()
        );
        if i < sel_index_inclusive {
            root_items_1.push(d_str);
        } else {
            root_items_2.push(d_str);
        }
    }

    let sublist_items = sel_event.map_or(vec![], |evt| {
        let mut items: Vec<_> = evt
            .data
            .applied_objs
            .iter()
            .zip(repeat("+"))
            .chain(evt.data.deleted_objs.iter().zip(repeat("-")))
            .map(|(obj, op)| format!("  {} {}", op, obj.namespaced_name()))
            .collect();
        if items.is_empty() {
            items.push(String::new());
        }
        items
    });

    let nested_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            // We know how many lines we have; use max constraints here so the lists are next to
            // each other.  The last one can be min(0) and take up the rest of the space
            Constraint::Max(root_items_1.len() as u16),
            Constraint::Max(sublist_items.len() as u16),
            Constraint::Min(0),
        ])
        .split(layout);

    let list_part_one = List::new(root_items_1)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");
    let sublist = List::new(sublist_items)
        .highlight_style(Style::new().bg(Color::Blue))
        .highlight_symbol("++ ")
        .style(Style::new().italic());
    let list_part_two = List::new(root_items_2).block(Block::new().padding(Padding::left(3)));

    frame.render_stateful_widget(list_part_one, nested_layout[0], &mut app.event_list_state);
    frame.render_stateful_widget(sublist, nested_layout[1], &mut app.object_list_state);
    frame.render_widget(list_part_two, nested_layout[2])
}

fn render_object(app: &mut App, frame: &mut Frame, layout: Rect) {
    let evt_idx = app.event_list_state.selected().unwrap();
    let obj_idx = app.object_list_state.selected().unwrap();
    let applied_len = app.trace.events[evt_idx].data.applied_objs.len();
    let deleted_len = app.trace.events[evt_idx].data.deleted_objs.len();

    let obj = if obj_idx >= applied_len {
        if obj_idx - applied_len > deleted_len {
            return;
        }
        &app.trace.events[evt_idx].data.deleted_objs[obj_idx - applied_len]
    } else {
        &app.trace.events[evt_idx].data.applied_objs[obj_idx]
    };

    let obj_str = serde_json::to_string_pretty(obj).unwrap();
    let contents = List::new(obj_str.split('\n')).highlight_style(Style::new().bg(Color::Blue));
    frame.render_stateful_widget(contents, layout, &mut app.object_contents_list_state);
}
