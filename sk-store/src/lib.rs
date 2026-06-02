#![cfg_attr(coverage, feature(coverage_attribute))]
mod event;
mod manager;
mod store;
mod watchers;

pub use crate::manager::TraceManager;
pub use crate::store::TraceStore;

#[cfg(test)]
mod tests;
