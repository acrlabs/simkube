use std::sync::{
    Arc,
    Mutex,
};

use clockabilly::mock::MockUtcClock;
use futures::stream;
use futures::stream::StreamExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::runtime::watcher::Event;
use kube::ResourceExt;
use sk_api::v1::ExportFilters;
use sk_core::macros::*;

use super::*;
use crate::watchers::{
    DynObjHandler,
    ObjStream,
    ObjWatcher,
};
use crate::TraceStore;

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

                // We recreate one of the pods at time zero just to make sure there's
                // no weird duplicate behaviours
                (0, id) => {
                    let obj = d(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Apply(obj)), (new_ts, id)));
                },

                // From times 10..20, we delete one of the regular pods
                (5..=19, id) => {
                    let obj = d(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Delete(obj)), (new_ts, id + 1)));
                },

                // In times 20..25, we test the various filter options:
                //  - a kube-system pod is created
                //  - a label-selector pod is created
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

                // Lastly we delete the remaining "regular" pods
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

mod itest {
    use super::*;

    #[rstest]
    #[case::full_trace(None)]
    #[case::partial_trace(Some("10s".into()))]
    #[traced_test]
    #[tokio::test]
    async fn itest_export(#[case] duration: Option<String>) {
        let clock = MockUtcClock::boxed(0);

        // Since we're just generating the results from the stream and not actually querying any
        // Kubernetes internals or whatever, the TracerConfig is empty.
        let s = Arc::new(Mutex::new(TraceStore::new(Default::default())));

        // First build up the stream of test data and run the watcher (this advances time to the "end")
        let h = DynObjHandler::new(DEPL_GVK.clone());
        let w = ObjWatcher::new_from_parts(h, test_stream(*clock.clone()), s.clone(), clock);
        w.start().await;

        // Next export the data with the chosen filters
        let filter = ExportFilters {
            excluded_namespaces: vec!["kube-system".into()],
            excluded_labels: vec![metav1::LabelSelector {
                match_labels: klabel!("foo" => "bar"),
                ..Default::default()
            }],
        };

        let store = s.lock().unwrap();
        let (start_ts, end_ts) = (15, 46);
        match store.export(start_ts, end_ts, &filter) {
            Ok(data) => {
                // Confirm that the results match what we expect
                let new_store = TraceStore::import(data, &duration).unwrap();
                let import_end_ts = duration.map(|_| start_ts + 10).unwrap_or(end_ts);
                let expected_pods = store.sorted_objs_at(import_end_ts, &filter);
                let actual_pods = new_store.sorted_objs_at(end_ts, &filter);
                println!("Expected pods: {:?}", expected_pods);
                println!("Actual pods: {:?}", actual_pods);
                assert_eq!(actual_pods, expected_pods);
            },
            Err(e) => panic!("failed with error: {}", e),
        };
    }
}
