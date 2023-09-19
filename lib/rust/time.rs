use chrono::Utc;

pub trait Clockable {
    fn now(&self) -> i64;
}

pub struct UtcClock {}

impl Clockable for UtcClock {
    fn now(&self) -> i64 {
        Utc::now().timestamp()
    }
}
