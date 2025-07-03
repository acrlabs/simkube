pub mod dyn_obj_watcher;
pub mod pod_watcher;

use std::collections::HashSet;
use std::mem::take;
use std::pin::Pin;
use std::sync::mpsc;
use std::sync::mpsc::{
    Receiver,
    Sender,
};

use async_trait::async_trait;
use clockabilly::prelude::*;
use futures::{
    Stream,
    StreamExt,
};
use kube::runtime::watcher::Event;
use sk_core::errors::*;
use tracing::*;

pub(super) type ObjStream<T> = Pin<Box<dyn Stream<Item = anyhow::Result<Event<T>>> + Send>>;

#[cfg_attr(test, automock)]
#[async_trait]
pub(crate) trait EventHandler<T: Clone + Send + Sync> {
    async fn applied(&mut self, obj: &T, ts: i64) -> EmptyResult;
    async fn deleted(&mut self, namespace: &str, name: &str, ts: i64) -> EmptyResult;
}

pub struct ObjWatcher<T: Clone + Send + Sync + kube::ResourceExt> {
    handler: Box<dyn EventHandler<T> + Send>,
    stream: ObjStream<T>,

    clock: Box<dyn Clockable + Send>,
    is_ready: bool,
    ready_tx: Sender<bool>,

    init_buffer: Vec<T>,
    index: HashSet<(String, String)>,
}

impl<T: Clone + Send + Sync + kube::ResourceExt> ObjWatcher<T> {
    fn new(handler: Box<dyn EventHandler<T> + Send>, stream: ObjStream<T>) -> (ObjWatcher<T>, Receiver<bool>) {
        let (tx, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        (
            ObjWatcher {
                handler,
                stream,

                clock: UtcClock::boxed(),
                is_ready: false,
                ready_tx: tx,

                init_buffer: vec![],
                index: HashSet::new(),
            },
            rx,
        )
    }

    // This is not a reference because it needs to "own" itself when tokio spawns it
    pub async fn start(mut self) {
        while let Some(res) = self.stream.next().await {
            let ts = self.clock.now_ts();
            match res {
                Ok(ref evt) => self.handle_event(evt, ts).await.unwrap_or_else(|err| {
                    // This error is "sortof" OK, in the sense that if we can't handle a single
                    // event, the tracer can potentially keep going on other events, so we don't
                    // display a stack trace here.
                    error!("could not handle event:\n\n{err}\n");
                }),
                Err(err) => {
                    // However, if there's a fundamental error getting something from the stream,
                    // the tracer can still maybe attempt to keep going, but that indicates
                    // somthing more problematic and program-stopping is going on, so we display a
                    // stack trace (using skerr).
                    skerr!(err, "pod watcher received error on stream");
                },
            }
        }
    }

    pub(crate) async fn handle_event(&mut self, evt: &Event<T>, ts: i64) -> EmptyResult {
        // We don't expect the trace store to panic, but if it does we should panic here too
        // (the unlock only fails here if the lock has been Poisoned, e.g., something panicked
        // while holding the lock)
        match evt {
            Event::Apply(obj) => {
                self.handler.applied(obj, ts).await?;
                self.index.insert((obj.namespace().unwrap_or_default(), obj.name_any()));
            },
            Event::Delete(obj) => {
                let ns = obj.namespace().unwrap_or_default();
                let name = obj.name_any();
                self.handler.deleted(&ns, &name, ts).await?;
                self.index.remove(&(ns, name));
            },
            Event::Init => (),
            Event::InitApply(obj) => self.init_buffer.push(obj.clone()),
            Event::InitDone => {
                // We swap the old index  for an (empty) new one, and remove events from the old
                // and putting them into the new.  Then we know that anything left in the old
                // after we're done was deleted in the intervening period.
                let mut old_objs = take(&mut self.index);
                for obj in &self.init_buffer {
                    self.handler.applied(obj, ts).await?;

                    let ns = obj.namespace().unwrap_or_default();
                    let name = obj.name_any();
                    old_objs.remove(&(ns, name));
                }

                for (ns, name) in old_objs {
                    self.handler.deleted(&ns, &name, ts).await?;
                }

                // When the watcher first starts up it does a List call, which (internally) gets
                // converted into a "Restarted" event that contains all of the listed objects.
                // Once we've handled this event the first time, we know we have a complete view of
                // the cluster at startup time.
                if !self.is_ready {
                    self.is_ready = true;

                    // unlike golang, sending is non-blocking
                    // if nobody's listening on the other end it's "fine" so we ignore the error
                    let _ = self.ready_tx.send(true);
                }
                self.init_buffer.clear();
            },
        }
        Ok(())
    }
}

#[cfg(test)]
use mockall::automock;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
impl<T: Clone + Send + Sync + kube::ResourceExt> ObjWatcher<T> {
    pub(crate) fn new_from_parts(
        handler: Box<dyn EventHandler<T> + Send>,
        stream: ObjStream<T>,
        clock: Box<dyn Clockable + Send>,
    ) -> ObjWatcher<T> {
        let (tx, _): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        ObjWatcher {
            handler,
            stream,
            clock,
            is_ready: true,
            ready_tx: tx,
            init_buffer: vec![],
            index: HashSet::new(),
        }
    }
}
