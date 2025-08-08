#![cfg_attr(coverage, feature(coverage_attribute))]
mod config;
mod event;
mod filter;
mod index;
mod manager;
mod pod_owners_map;
mod store;
mod trace;
mod watchers;

pub use crate::config::{
    TracerConfig,
    TrackedObjectConfig,
};
pub use crate::event::{
    TraceAction,
    TraceEvent,
};
pub use crate::index::TraceIndex;
pub use crate::manager::TraceManager;
pub use crate::pod_owners_map::PodLifecyclesMap;
pub use crate::store::TraceStore;
pub use crate::trace::{
    ExportedTrace,
    TraceIterator,
};

const CURRENT_TRACE_FORMAT_VERSION: u16 = 2;

#[cfg(test)]
mod tests;
