mod dyn_obj_watcher;
mod pod_watcher;

use std::pin::Pin;

use futures::Stream;
use k8s_openapi::api::core::v1 as corev1;
use kube::api::DynamicObject;
use kube::runtime::watcher::Event;

pub type KubeObjectStream = Pin<Box<dyn Stream<Item = anyhow::Result<Event<DynamicObject>>> + Send>>;
pub type PodStream = Pin<Box<dyn Stream<Item = anyhow::Result<Event<corev1::Pod>>> + Send>>;

pub use self::dyn_obj_watcher::DynObjWatcher;
pub use self::pod_watcher::PodWatcher;

#[cfg(test)]
mod tests;
