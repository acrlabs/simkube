use chrono::TimeDelta;
use lazy_static::lazy_static;
use ratatui::prelude::*;
use sk_core::prelude::*;

use crate::validation::{
    AnnotatedTraceEvent,
    ValidatorType,
};

pub(super) const LIST_PADDING: usize = 3;
lazy_static! {
    static ref ERR_STYLE: Style = Style::new().white().on_red();
    static ref WARN_STYLE: Style = Style::new().white().on_yellow();
}

pub(super) fn make_event_spans(event: &AnnotatedTraceEvent, start_ts: i64) -> (Span<'_>, Span<'_>) {
    let d = TimeDelta::new(event.data.ts - start_ts, 0).unwrap();
    let evt_span = Span::from(format!(
        "{} ({} applied/{} deleted)",
        format_duration(d),
        event.data.applied_objs.len(),
        event.data.deleted_objs.len()
    ));

    let (warnings, errs) = event.annotations.values().fold((0, 0), |(mut w, mut e), annotations| {
        for a in annotations {
            match a.code.0 {
                ValidatorType::Warning => w += 1,
                ValidatorType::Error => e += 1,
            }
        }
        (w, e)
    });

    if warnings + errs == 0 {
        (evt_span, Span::default())
    } else {
        let err_str = format!("{} error{}", errs, if errs == 1 { "" } else { "s" });
        let warn_str = format!("{} warning{}", warnings, if warnings == 1 { "" } else { "s" });
        let style = if errs == 0 { *WARN_STYLE } else { *ERR_STYLE };
        let err_span = Span::styled(format!(" {err_str}/{warn_str} "), style);

        (evt_span, err_span)
    }
}

pub(super) fn make_object_spans<'a>(
    index: usize,
    obj: &DynamicObject,
    op: char,
    event: &'a AnnotatedTraceEvent,
) -> (Span<'a>, Span<'a>) {
    let obj_span = Span::styled(format!("  {} {}", op, obj.namespaced_name()), Style::new().italic());
    let (warnings, errs) = event
        .annotations
        .get(&index)
        .unwrap_or(&vec![])
        .iter()
        .fold((0, 0), |(mut w, mut e), a| {
            match a.code.0 {
                ValidatorType::Warning => w += 1,
                ValidatorType::Error => e += 1,
            }
            (w, e)
        });
    if warnings + errs == 0 {
        (obj_span, Span::default())
    } else {
        let err_str = format!("{} error{}", errs, if errs == 1 { "" } else { "s" });
        let warn_str = format!("{} warning{}", warnings, if warnings == 1 { "" } else { "s" });
        let style = if errs == 0 { *WARN_STYLE } else { *ERR_STYLE };
        let err_span = Span::styled(format!(" {err_str}/{warn_str} "), style);

        (obj_span, err_span)
    }
}

pub(super) fn format_list_entries<'a>(
    spans: impl Iterator<Item = (Span<'a>, Span<'a>)> + Clone,
    width: usize,
) -> Vec<Text<'a>> {
    let max_err_width = spans
        .clone()
        .max_by_key(|(_, err_span)| err_span.width())
        .map_or(0, |(_, err_span)| err_span.width());

    spans
        .map(|(mut item_span, err_span)| {
            // Instead of using a table we're just computing everything by hand here.  The format
            // of a list entry is LIST_PADDING|item|spaces|error|LIST_PADDING
            //
            // In order to ensure that all the error lines are the same width, we compute the
            // maximum above, and then use that everywhere.
            //
            // If the length of the item + the error + 2 * LIST_PADDING > width of the screen,
            // we have no spaces in the middle.  That's what these saturating subs are computing.
            // On the other hand, if there's no error to display, then we just fill out the rest of
            // the line with spaces.
            let mid_padding_width = width
                .saturating_sub(item_span.width())
                .saturating_sub(max_err_width)
                .saturating_sub(LIST_PADDING * 2);
            let mid_padding_span = Span::from(" ".repeat(mid_padding_width));

            // We want the right padding to match the same style as the error or warning, so if an
            // error is present we create a separate span here.
            let (trunc_width, right_padding_width) = match err_span.width() {
                0 => (width - LIST_PADDING, 0),
                x => (width - (mid_padding_width + max_err_width + LIST_PADDING), max_err_width - x + LIST_PADDING),
            };
            let right_padding_span = Span::styled(" ".repeat(right_padding_width), err_span.style);

            // If the list entry is too long, then we truncate it and replace the rightmost
            // characters with an ellipsis.
            if trunc_width < item_span.content.len() {
                let content = item_span.content.to_mut();
                content.truncate(trunc_width);
                content.replace_range(trunc_width - 4..trunc_width, "... ");
            }

            Text::from(item_span.clone() + mid_padding_span + err_span.clone() + right_padding_span)
        })
        .collect()
}

pub(super) fn format_duration(d: TimeDelta) -> String {
    let day_str = match d.num_days() {
        x if x > 0 => format!("{x}d "),
        _ => String::new(),
    };

    format!("{}{:02}:{:02}:{:02}", day_str, d.num_hours() % 24, d.num_minutes() % 60, d.num_seconds() % 60)
}
