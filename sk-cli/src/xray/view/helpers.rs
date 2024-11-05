use chrono::TimeDelta;
use kube::api::DynamicObject;
use lazy_static::lazy_static;
use ratatui::prelude::*;
use sk_core::k8s::KubeResourceExt;

use crate::validation::{
    AnnotatedTraceEvent,
    ValidatorType,
};

pub(super) const LIST_PADDING: usize = 3;
lazy_static! {
    static ref ERR_STYLE: Style = Style::new().white().on_red();
    static ref WARN_STYLE: Style = Style::new().white().on_yellow();
}

pub(super) fn make_event_spans(event: &AnnotatedTraceEvent, start_ts: i64) -> (Span, Span) {
    let d = TimeDelta::new(event.data.ts - start_ts, 0).unwrap();
    let evt_span = Span::from(format!(
        "{} ({} applied/{} deleted)",
        format_duration(d),
        event.data.applied_objs.len(),
        event.data.deleted_objs.len()
    ));

    let (warnings, errs) = event.annotations.iter().fold((0, 0), |(mut w, mut e), codes| {
        for code in codes {
            match code.0 {
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
    let (warnings, errs) = event.annotations[index].iter().fold((0, 0), |(mut w, mut e), code| {
        match code.0 {
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
        .map(|(evt_span, err_span)| {
            let mid_padding_width = width - evt_span.width() - max_err_width - (LIST_PADDING * 2);
            let mid_padding_span = Span::from(" ".repeat(mid_padding_width));
            let right_padding_width = match err_span.width() {
                0 => 0,
                x => max_err_width - x + LIST_PADDING,
            };
            let right_padding_span = Span::styled(" ".repeat(right_padding_width), err_span.style);

            Text::from(evt_span.clone() + mid_padding_span + err_span.clone() + right_padding_span)
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
