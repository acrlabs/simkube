use kube::runtime::watcher::Event;

pub(super) trait TryModify<K> {
    fn try_modify(self, f: impl FnMut(&mut K) -> anyhow::Result<()>) -> anyhow::Result<Event<K>>;
}

impl<K> TryModify<K> for Event<K> {
    fn try_modify(mut self, mut f: impl FnMut(&mut K) -> anyhow::Result<()>) -> anyhow::Result<Event<K>> {
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
