use chrono::{
    DateTime,
    Local,
    Utc,
};
use parse_datetime::{
    parse_datetime,
    parse_datetime_at_date,
};

// This trait exists for testing, so that we can provide consistent timestamp values to objects
// instead of just relying on whatever the current time actually is.

pub trait Clockable {
    fn now(&self) -> DateTime<Utc>;
    fn now_ts(&self) -> i64;
}

pub struct UtcClock;

impl UtcClock {
    pub fn new() -> Box<UtcClock> {
        Box::new(UtcClock)
    }
}

impl Clockable for UtcClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn now_ts(&self) -> i64 {
        Utc::now().timestamp()
    }
}

pub fn duration_to_ts(tstr: &str) -> anyhow::Result<i64> {
    Ok(parse_datetime(tstr)?.timestamp())
}

pub fn duration_to_ts_from(start_ts: i64, tstr: &str) -> anyhow::Result<i64> {
    let local_time = DateTime::from_timestamp(start_ts, 0).unwrap().with_timezone(&Local);
    Ok(parse_datetime_at_date(local_time, tstr)?.timestamp())
}
