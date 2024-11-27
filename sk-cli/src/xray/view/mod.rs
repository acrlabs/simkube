mod helpers;

use std::cmp::{
    max,
    min,
};
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
    ListState,
    Padding,
    Paragraph,
};

use self::helpers::*;
use super::app::{
    App,
    JumpDir,
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
    let (events_inner, object_inner);

    if top.width > MIN_TWO_PANEL_WIDTH {
        let lr_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(top);
        let (left, right) = (lr_layout[0], lr_layout[1]);

        events_inner = events_border.inner(left);
        object_inner = object_border.inner(right);
        render_event_list(app, frame, events_inner);
        if app.mode == Mode::EventSelected || app.mode == Mode::ObjectSelected {
            render_object(app, frame, object_inner);
        }

        frame.render_widget(events_border, left);
        frame.render_widget(object_border, right);
    } else {
        events_inner = events_border.inner(top);
        render_event_list(app, frame, events_inner);
        frame.render_widget(events_border, top);

        // We compute the popup dimensions so we can cleanly forward-declare object_inner
        // above, but we don't actually render anything unless we're in the right mode.
        let popup = Rect {
            x: top.width / 10,
            y: top.height / 10,
            width: 4 * top.width / 5,
            height: 4 * top.height / 5,
        };
        object_inner = object_border.inner(popup);

        if app.mode == Mode::ObjectSelected {
            frame.render_widget(Clear, popup);
            render_object(app, frame, object_inner);
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
    let hi_index = app.highlighted_event_index();
    let (sel_index_inclusive, sel_event) = match app.mode {
        Mode::EventSelected | Mode::ObjectSelected => (hi_index + 1, app.annotated_trace.get_event(hi_index)),
        _ => (num_events, None),
    };

    let event_spans = app.annotated_trace.iter().map(|event| make_event_spans(event, start_ts));
    let mut top_entries = format_list_entries(event_spans, layout.width as usize);
    let bottom_entries = top_entries.split_off(sel_index_inclusive);

    // chain together the applied and deleted objects with either a "+" or "-" prefix, respectively
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
            // if there are no objects associated with this event, we display an empty span
            format_list_entries(once((Span::default(), Span::default())), layout.width as usize)
        } else {
            format_list_entries(sublist_items, layout.width as usize)
        }
    });

    // Compute the constraint values for the first part of the events list and the objects list
    // AND ALSO update the selected and offset pointers if we've jumped; we have to interweave
    // these because we base the jump distance on the currently viewed events and objects
    let (top_height, mid_height) = if app.mode == Mode::RootView {
        if let Some(dir) = app.jump {
            jump_list_state(&mut app.event_list_state, dir, top_entries.len(), layout.height, false);
        }
        // If we're in the root view, the number of entries to display is just the total number of
        // events minus the current view offset.  Since there is no selected event, the number of
        // middle entries is 0.
        (top_entries.len().saturating_sub(app.event_list_state.offset()) as u16, 0)
    } else {
        // If we've selected an event to view, we have to first compute the height ot the top list
        // as above; the length of the middle list is either:
        //   - 1 if the selected event is at the very bottom of the display (which will push the offset of
        //     the top list up by one)
        //   - the number of rows between the selected event and the bottom of the display, OR
        //   - the total number of objects belonging to this event (if they all fit)
        let th = top_entries.len().saturating_sub(app.event_list_state.offset()) as u16;
        let mh = min(
            max(1, layout.height.saturating_sub(th)),
            obj_spans.len().saturating_sub(app.object_list_state.offset()) as u16,
        );

        // Once we know how many objects we can display for this event, we can compute the amount
        // to jump (if any); the key observation that makes this all work is that if we've selected
        // an event, the length of the top entries is fixed.
        if app.mode == Mode::EventSelected {
            if let Some(dir) = app.jump {
                jump_list_state(&mut app.object_list_state, dir, obj_spans.len(), mh, true);
            };
        }
        (th, mh)
    };

    // We know how many lines we have; use max constraints here so the lists are next to
    // each other.  The last one can be min(0) and take up the rest of the space
    let nested_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Max(top_height), Constraint::Max(mid_height), Constraint::Min(0)])
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

    if app.mode == Mode::ObjectSelected {
        if let Some(dir) = app.jump {
            jump_list_state(&mut app.object_contents_list_state, dir, contents.len(), layout.height, false);
        }
    }

    frame.render_stateful_widget(contents, layout, &mut app.object_contents_list_state);
}

fn jump_list_state(list_state: &mut ListState, jump_dir: JumpDir, list_len: usize, view_height: u16, pin_bottom: bool) {
    // compute how far to jump in the specified direction, given the number of items in the
    // list that we're trying to display and how much room we have to display them
    let offset = list_state.offset();
    let selected = list_state.selected().unwrap();

    let new_pos = match jump_dir {
        // If we jump down, we increase the selection by the view height - 1, so that the
        // last-visible item before the jump becomes the first-visible item after the jump
        JumpDir::Down => (offset + view_height as usize).saturating_sub(1),

        // If we jump up, and we aren't on the first item, just jump to the top of the current
        // page; otherwise, page up, but subtract one so that the top item before the jump
        // becomes the bottom item after the jump
        JumpDir::Up => {
            if selected != offset {
                offset
            } else {
                // make sure the -1 is _inside_ the saturating_sub, instead of adding 1 _outside_;
                // otherwise you get weird behaviours when selected == offset == 0.
                offset.saturating_sub(view_height as usize - 1)
            }
        },
    };

    // we're relying on the ratatui behaviour of adjusting the selected index to be "within bounds"
    // if we computed something out of bounds above.
    list_state.select(Some(new_pos));

    // pin_bottom means that we don't want to have empty space at the bottom if we jumped "too
    // far", so we make a special (smaller) offset computation in this case
    if pin_bottom && list_len.saturating_sub(new_pos) < view_height as usize {
        *list_state.offset_mut() = list_len.saturating_sub(view_height as usize);
    } else if new_pos < list_len {
        // otherwise, we just set the new offset to the selected item, so that it appears at the
        // top of the page
        *list_state.offset_mut() = new_pos;
    }
}

#[cfg(test)]
mod tests;
