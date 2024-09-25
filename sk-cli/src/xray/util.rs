use chrono::TimeDelta;

pub(super) fn format_duration(d: TimeDelta) -> String {
    let day_str = match d.num_days() {
        x if x > 0 => format!("{x}d "),
        _ => String::new(),
    };

    format!("{}{:02}:{:02}:{:02}", day_str, d.num_hours() % 24, d.num_minutes() % 60, d.num_seconds() % 60)
}
