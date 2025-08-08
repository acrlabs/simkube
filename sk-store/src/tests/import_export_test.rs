use std::collections::HashSet;
use std::sync::{
    Arc,
    Mutex,
};

use assertables::*;
use clockabilly::mock::MockUtcClock;
use futures::stream;
use futures::stream::StreamExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::ResourceExt;
use kube::runtime::watcher::Event;
use sk_api::v1::ExportFilters;
use sk_core::k8s::{
    GVK,
    format_gvk_name,
};
use sk_core::macros::*;
use tokio::sync::mpsc;

use super::*;
use crate::TraceStore;
use crate::manager::handle_messages;
use crate::watchers::{
    ObjStream,
    dyn_obj_watcher,
    pod_watcher,
};

fn d(idx: i64) -> DynamicObject {
    test_deployment(&format!("depl{idx}"))
}

// Set up a test stream to ensure that imports and exports work correctly.
//
// We use stream::unfold to build a stream from a set of events; the unfold takes a "state" tuple
// as input, which has the "current timestamp" and "id of next regular pod to delete" as input.
//
// This is a little subtle because the event that we're returning at state (ts_i, id) does not
// actually _happen_ until time ts_{i+1}.
fn test_stream(clock: MockUtcClock) -> ObjStream<DynamicObject> {
    stream::unfold((-1, -1), move |state| {
        let mut c = clock.clone();
        async move {
            match state {
                // Initial conditions: we create 10 deployments
                (-1, -1) => {
                    return Some((Ok(Event::Init), (-1, 0)));
                },

                (-1, id) if id < 10 => {
                    let obj = d(id);
                    return Some((Ok(Event::InitApply(obj)), (-1, id + 1)));
                },

                (-1, 10) => {
                    return Some((Ok(Event::InitDone), (0, 0)));
                },

                // We recreate one of the deployments at time zero just to make sure there's
                // no weird duplicate behaviours
                (0, id) => {
                    let obj = d(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Apply(obj)), (new_ts, id)));
                },

                // From times 10..20, we delete one of the regular deployments
                (5..=19, id) => {
                    let obj = d(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Delete(obj)), (new_ts, id + 1)));
                },

                // In times 20..25, we test the various filter options:
                //  - a kube-system deploymnet is created
                //  - a label-selector deployment is created
                //
                // In the test below, all of these events should be filtered out
                (22, id) => {
                    let mut obj = d(30);
                    obj.metadata.namespace = Some("kube-system".into());
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Apply(obj)), (new_ts, id)));
                },
                (24, id) => {
                    let mut obj = d(31);
                    obj.labels_mut().insert("foo".into(), "bar".into());
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Apply(obj)), (new_ts, id)));
                },

                // Lastly we delete the remaining "regular" deployments
                (25..=55, id) => {
                    let obj = d(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Delete(obj)), (new_ts, id + 1)));
                },
                _ => None,
            }
        }
    })
    .boxed()
}

fn objs_in_trace(trace: &ExportedTrace) -> HashSet<String> {
    let mut objs = HashSet::new();
    for evt in &trace.events {
        for obj in &evt.applied_objs {
            objs.insert(format_gvk_name(&GVK::from_dynamic_obj(&obj).unwrap(), &obj.namespaced_name()));
        }
        for obj in &evt.deleted_objs {
            objs.remove(&format_gvk_name(&GVK::from_dynamic_obj(&obj).unwrap(), &obj.namespaced_name()));
        }
    }
    objs
}

mod itest {
    use super::*;

    #[rstest(tokio::test)]
    #[case::full_trace(None)]
    #[case::partial_trace(Some("10s".into()))]
    async fn test_export(#[case] duration: Option<String>) {
        let clock = MockUtcClock::boxed(0);

        // Since we're just generating the results from the stream and not actually querying any
        // Kubernetes internals or whatever, the TracerConfig is empty.
        let config = TracerConfig::default();
        let s = Arc::new(Mutex::new(TraceStore::new(config.clone())));
        let (dyn_obj_tx, dyn_obj_rx): (dyn_obj_watcher::Sender, dyn_obj_watcher::Receiver) = mpsc::unbounded_channel();
        let (_, pod_rx): (pod_watcher::Sender, pod_watcher::Receiver) = mpsc::unbounded_channel();

        // First build up the stream of test data and run the watcher (this advances time to the "end")
        let (ready_tx, _): (mpsc::Sender<bool>, mpsc::Receiver<bool>) = mpsc::channel(1);
        let w =
            dyn_obj_watcher::new_from_parts(DEPL_GVK.clone(), dyn_obj_tx, test_stream(*clock.clone()), clock, ready_tx);
        w.start().await;

        // Next "handle" all the messages that the watcher sent
        handle_messages(dyn_obj_rx, pod_rx, s.clone()).await;

        // Next export the data with the chosen filters
        let filter = ExportFilters {
            excluded_namespaces: vec!["kube-system".into()],
            excluded_labels: vec![metav1::LabelSelector {
                match_labels: klabel!("foo" => "bar"),
                ..Default::default()
            }],
        };

        let (start_ts, end_ts) = (15, 46);
        let store = s.lock().unwrap();
        match store.export(start_ts, end_ts, &filter) {
            Ok(data) => {
                // Confirm that the results match what we expect
                let trace = ExportedTrace::import(data, duration.as_ref()).unwrap();
                let import_end_ts = duration.map(|_| start_ts + 10).unwrap_or(end_ts);
                let expected_objs = store.objs_at(import_end_ts, &filter);
                let actual_objs = objs_in_trace(&trace);

                assert_bag_eq!(actual_objs, expected_objs);
            },
            Err(e) => panic!("failed with error: {}", e),
        };
    }
}
