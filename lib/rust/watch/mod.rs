mod watcher;

use std::pin::Pin;

use futures::Stream;
use k8s_openapi::api::core::v1 as corev1;
use kube::api::DynamicObject;
use kube::runtime::watcher::Event;

pub enum KubeEvent {
    Dyn(Event<DynamicObject>),
    Pod(Event<corev1::Pod>),
}

pub type KubeObjectStream<'a> = Pin<Box<dyn Stream<Item = anyhow::Result<KubeEvent>> + Send + 'a>>;

pub use self::watcher::Watcher;
