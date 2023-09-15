mod watcher;

use std::pin::Pin;

use futures::Stream;
use kube::api::DynamicObject;
use kube::runtime::watcher::Event;

pub type KubeObjectStream<'a> = Pin<Box<dyn Stream<Item = anyhow::Result<Event<DynamicObject>>> + Send + 'a>>;

pub use self::watcher::Watcher;
