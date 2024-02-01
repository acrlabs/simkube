use chrono::Utc;
use parse_datetime::parse_datetime;

// This trait exists for testing, so that we can provide consistent timestamp values to objects
// instead of just relying on whatever the current time actually is.

pub trait Clockable {
    fn now(&self) -> i64;
}

pub struct UtcClock;

impl Clockable for UtcClock {
    fn now(&self) -> i64 {
        Utc::now().timestamp()
    }
}

pub fn parse(tstr: &str) -> anyhow::Result<i64> {
    Ok(parse_datetime(tstr)?.timestamp())
}
