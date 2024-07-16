pub use chrono::*;

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

#[cfg(feature = "mock")]
pub mod mock {
    use std::sync::atomic::{
        AtomicI64,
        Ordering,
    };
    use std::sync::Arc;

    use super::*;

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
        fn now(&self) -> DateTime<Utc> {
            return DateTime::from_timestamp(self.now_ts(), 0).unwrap();
        }

        fn now_ts(&self) -> i64 {
            return self.now.load(Ordering::Relaxed);
        }
    }
}
