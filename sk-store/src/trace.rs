use std::collections::HashMap;

use anyhow::bail;
use clockabilly::prelude::*;
use serde::{
    Deserialize,
    Serialize,
};
use sk_core::k8s::{
    GVK,
    PodLifecycleData,
};
use sk_core::time::duration_to_ts_from;
use thiserror::Error;
use tracing::*;

use crate::CURRENT_TRACE_FORMAT_VERSION;
use crate::config::TracerConfig;
use crate::event::TraceEvent;
use crate::index::TraceIndex;
use crate::pod_owners_map::PodLifecyclesMap;

#[derive(Debug, Error)]
pub enum TraceError {
    #[error(
        "could not parse trace file\n\nIf this trace file is older than version 2, \
        it is only parseable by SimKube <= 1.1.1.  Please see the release notes for details."
    )]
    ParseFailed(#[from] rmp_serde::decode::Error),
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ExportedTrace {
    pub version: u16,
    pub config: TracerConfig,
    pub events: Vec<TraceEvent>,
    pub index: TraceIndex,
    pub pod_lifecycles: HashMap<(GVK, String), PodLifecyclesMap>,
}

impl Default for ExportedTrace {
    fn default() -> Self {
        ExportedTrace {
            version: CURRENT_TRACE_FORMAT_VERSION,
            config: TracerConfig::default(),
            events: vec![],
            index: TraceIndex::default(),
            pod_lifecycles: HashMap::default(),
        }
    }
}

impl ExportedTrace {
    pub fn import(data: Vec<u8>, maybe_duration: Option<&String>) -> anyhow::Result<ExportedTrace> {
        let mut exported_trace = rmp_serde::from_slice::<ExportedTrace>(&data).map_err(TraceError::ParseFailed)?;

        if exported_trace.version != CURRENT_TRACE_FORMAT_VERSION {
            bail!("unsupported trace version: {}", exported_trace.version);
        }

        let trace_start_ts = exported_trace
            .events
            .first()
            .unwrap_or(&TraceEvent { ts: UtcClock.now_ts(), ..Default::default() })
            .ts;
        let mut trace_end_ts = exported_trace
            .events
            .last()
            .unwrap_or(&TraceEvent { ts: UtcClock.now_ts(), ..Default::default() })
            .ts;
        if let Some(trace_duration_str) = maybe_duration {
            trace_end_ts = duration_to_ts_from(trace_start_ts, trace_duration_str)?;
            exported_trace.events.retain(|evt| evt.ts < trace_end_ts);

            // Add an empty event to the very end to make sure the driver doesn't shut down early
            exported_trace
                .events
                .push(TraceEvent { ts: trace_end_ts, ..Default::default() });
        }

        info!("Imported {} events between {trace_start_ts} and {trace_end_ts}", exported_trace.events.len());
        Ok(exported_trace)
    }

    pub fn clone_with_events(&self, events: Vec<TraceEvent>) -> ExportedTrace {
        let mut trace = self.clone();
        trace.events = events;
        trace
    }

    pub fn lookup_pod_lifecycle(
        &self,
        owner_gvk: &GVK,
        owner_ns_name: &str,
        pod_hash: u64,
        seq: usize,
    ) -> PodLifecycleData {
        let maybe_lifecycle_data = self
            .pod_lifecycles
            .get(&(owner_gvk.clone(), owner_ns_name.into()))
            .and_then(|l| l.get(&pod_hash));
        match maybe_lifecycle_data {
            Some(data) => data[seq % data.len()].clone(),
            _ => PodLifecycleData::Empty,
        }
    }

    pub fn append_event(&mut self, event: TraceEvent) {
        self.events.push(event);
    }

    pub fn end_ts(&self) -> Option<i64> {
        self.events.last().map(|evt| evt.ts)
    }

    pub fn events(&self) -> Vec<TraceEvent> {
        self.events.clone()
    }

    pub fn has_obj(&self, gvk: &GVK, ns_name: &str) -> bool {
        self.index.contains(gvk, ns_name)
    }

    pub fn iter(&self) -> TraceIterator<'_> {
        TraceIterator::new(&self.events)
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn prepend_event(&mut self, event: TraceEvent) {
        let mut tmp = vec![event];
        tmp.append(&mut self.events);
        self.events = tmp;
    }

    pub fn start_ts(&self) -> Option<i64> {
        self.events.first().map(|evt| evt.ts)
    }

    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }
}

pub struct TraceIterator<'a> {
    events: &'a Vec<TraceEvent>,
    idx: usize,
}

impl<'a> TraceIterator<'a> {
    pub(crate) fn new(events: &'a Vec<TraceEvent>) -> Self {
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
