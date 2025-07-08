use crate::event::{
    TraceEvent,
    TraceEventList,
};

pub struct TraceIterator<'a> {
    events: &'a TraceEventList,
    idx: usize,
}

impl<'a> TraceIterator<'a> {
    pub(crate) fn new(events: &'a TraceEventList) -> Self {
        TraceIterator { events, idx: 0 }
    }
}


// Our iterator implementation iterates over all the events in timeseries order.  It returns the
// current event, and the timestamp of the _next_ event.
impl<'a> Iterator for TraceIterator<'a> {
    type Item = (&'a TraceEvent, Option<i64>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.events.is_empty() {
            return None;
        }

        let ret = match self.idx {
            i if i < self.events.len() - 1 => Some((&self.events[i], Some(self.events[i + 1].ts))),
            i if i == self.events.len() - 1 => Some((&self.events[i], None)),
            _ => None,
        };

        self.idx += 1;
        ret
    }
}
