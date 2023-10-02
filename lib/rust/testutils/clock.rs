use std::sync::atomic::{
    AtomicI64,
    Ordering,
};
use std::sync::Arc;

use crate::time::Clockable;

#[derive(Clone)]
pub struct MockUtcClock {
    now: Arc<AtomicI64>,
}

impl MockUtcClock {
    pub fn new(start_ts: i64) -> Box<MockUtcClock> {
        Box::new(MockUtcClock { now: Arc::new(AtomicI64::new(start_ts)) })
    }

    pub fn advance(&mut self, duration: i64) -> i64 {
        let old = self.now.fetch_add(duration, Ordering::Relaxed);
        old + duration
    }

    pub fn set(&mut self, ts: i64) -> i64 {
        self.now.store(ts, Ordering::Relaxed);
        ts
    }
}

impl Clockable for MockUtcClock {
    fn now(&self) -> i64 {
        return self.now.load(Ordering::Relaxed);
    }
}
