use kube::runtime::watcher::Event;

use crate::prelude::*;

pub(super) fn try_modify<K>(
    mut e: Event<K>,
    mut f: impl FnMut(&mut K) -> SimKubeResult<()>,
) -> SimKubeResult<Event<K>> {
    match &mut e {
        Event::Applied(obj) | Event::Deleted(obj) => (f)(obj)?,
        Event::Restarted(objs) => {
            for k in objs {
                (f)(k)?
            }
        },
    }
    Ok(e)
}
