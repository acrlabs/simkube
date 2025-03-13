use clockabilly::{DateTime, Local};
use parse_datetime_fork::{parse_datetime, parse_datetime_at_date};

pub fn duration_to_ts(tstr: &str) -> anyhow::Result<i64> {
    Ok(parse_datetime(tstr)?.timestamp())
}

pub fn duration_to_ts_from(start_ts: i64, tstr: &str) -> anyhow::Result<i64> {
    let local_time = DateTime::from_timestamp(start_ts, 0).unwrap().with_timezone(&Local);
    Ok(parse_datetime_at_date(local_time, tstr)?.timestamp())
}
