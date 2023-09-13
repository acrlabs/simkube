use std::error::Error;

use kube::runtime::watcher::Event;

pub(super) trait TryModify<K, E: Error> {
    fn try_modify(self, f: impl FnMut(&mut K) -> Result<(), E>) -> Result<Event<K>, E>;
}

impl<K, E: Error> TryModify<K, E> for Event<K> {
    fn try_modify(mut self, mut f: impl FnMut(&mut K) -> Result<(), E>) -> Result<Event<K>, E> {
        match &mut self {
            Event::Applied(obj) | Event::Deleted(obj) => (f)(obj)?,
            Event::Restarted(objs) => {
                for k in objs {
                    (f)(k)?
                }
            },
        }
        Ok(self)
    }
}
