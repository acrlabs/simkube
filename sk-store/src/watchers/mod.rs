mod dyn_obj_watcher;
mod pod_watcher;

use std::pin::Pin;
use std::sync::mpsc::{
    Receiver,
    Sender,
};
use std::sync::{
    mpsc,
    Arc,
    Mutex,
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

pub use self::dyn_obj_watcher::DynObjHandler;
pub use self::pod_watcher::PodHandler;
use crate::TraceStorable;

pub type ObjStream<T> = Pin<Box<dyn Stream<Item = anyhow::Result<Event<T>>> + Send>>;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait EventHandler<T: Clone + Send + Sync> {
    async fn applied(&mut self, obj: &T, ts: i64, store: Arc<Mutex<dyn TraceStorable + Send>>) -> EmptyResult;
    async fn deleted(&mut self, obj: &T, ts: i64, store: Arc<Mutex<dyn TraceStorable + Send>>) -> EmptyResult;
    async fn initialized(&mut self, objs: &[T], ts: i64, store: Arc<Mutex<dyn TraceStorable + Send>>) -> EmptyResult;
}

pub struct ObjWatcher<T: Clone> {
    handler: Box<dyn EventHandler<T> + Send>,

    stream: ObjStream<T>,
    store: Arc<Mutex<dyn TraceStorable + Send>>,

    clock: Box<dyn Clockable + Send>,
    is_ready: bool,
    ready_tx: Sender<bool>,

    init_buffer: Vec<T>,
}

impl<T: Clone + Send + Sync> ObjWatcher<T> {
    pub fn new(
        handler: Box<dyn EventHandler<T> + Send>,
        stream: ObjStream<T>,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> (ObjWatcher<T>, Receiver<bool>) {
        let (tx, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        (
            ObjWatcher {
                handler,

                stream,
                store,

                clock: UtcClock::boxed(),
                is_ready: false,
                ready_tx: tx,

                init_buffer: vec![],
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
            Event::Apply(obj) => self.handler.applied(obj, ts, self.store.clone()).await?,
            Event::Delete(obj) => self.handler.deleted(obj, ts, self.store.clone()).await?,
            Event::Init => (),
            Event::InitApply(obj) => self.init_buffer.push(obj.clone()),
            Event::InitDone => {
                self.handler.initialized(&self.init_buffer, ts, self.store.clone()).await?;

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
impl<T: Clone> ObjWatcher<T> {
    pub fn new_from_parts(
        handler: Box<dyn EventHandler<T> + Send>,
        stream: ObjStream<T>,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
        clock: Box<dyn Clockable + Send>,
    ) -> ObjWatcher<T> {
        let (tx, _): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        ObjWatcher {
            handler,
            stream,
            store,
            clock,
            is_ready: true,
            ready_tx: tx,
            init_buffer: vec![],
        }
    }
}
