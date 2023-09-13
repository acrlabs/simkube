use std::collections::BTreeMap;
use std::sync::{
    Arc,
    Mutex,
};

use futures::stream;
use futures::stream::StreamExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use kube::runtime::watcher::Event;
use kube::ResourceExt;
use serde_json::Value;
use simkube::trace::{
    TraceFilter,
    Tracer,
};
use simkube::util::Clockable;
use simkube::watch::{
    KubeObjectStream,
    Watcher,
};

const TESTING_NAMESPACE: &str = "test";

struct MockUtcClock {
    now: i64,
}

impl MockUtcClock {
    fn advance(&mut self, duration: i64) -> i64 {
        self.now += duration;
        self.now
    }
}

impl Clockable for MockUtcClock {
    fn now(&self) -> i64 {
        return self.now;
    }
}

fn test_pod(idx: i64) -> DynamicObject {
    return DynamicObject {
        metadata: metav1::ObjectMeta {
            namespace: Some(TESTING_NAMESPACE.into()),
            name: Some(format!("pod{}", idx).into()),
            ..Default::default()
        },
        types: None,
        data: Value::Null,
    };
}

fn test_daemonset_pod(idx: i64) -> DynamicObject {
    let mut ds_pod = test_pod(idx + 100);
    ds_pod.metadata.owner_references =
        Some(vec![metav1::OwnerReference { kind: "DaemonSet".into(), ..Default::default() }]);
    ds_pod
}

// Set up a test stream to ensure that imports and exports work correctly.
//
// We use stream::unfold to build a stream from a set of events; the unfold takes a "state" tuple
// as input, which has the "current timestamp" and "id of next regular pod to delete" as input.
//
// This is a little subtle because the event that we're returning at state (ts_i, id) does not
// actually _happen_ until time ts_{i+1}.
fn test_stream(clock: Arc<Mutex<MockUtcClock>>) -> KubeObjectStream<'static> {
    return stream::unfold((-1, 0), move |state| {
        let clock = clock.clone();
        async move {
            let mut c = clock.lock().unwrap();
            match state {
                // Initial conditions: we create 10 regular pods and 5 daemonset pods
                (-1, id) => {
                    let mut pods: Vec<_> = (0..10).map(|i| test_pod(i)).collect();
                    let ds_pods: Vec<_> = (0..5).map(|i| test_daemonset_pod(i)).collect();
                    pods.extend(ds_pods);
                    return Some((Ok(Event::Restarted(pods)), (0, id)));
                },

                // We recreate one of the pods at time zero just to make sure there's
                // no weird duplicate behaviours
                (0, id) => {
                    let pod = test_pod(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Applied(pod)), (new_ts, id)));
                },

                // From times 10..20, we delete one of the regular pods
                (5..=19, id) => {
                    let pod = test_pod(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Deleted(pod)), (new_ts, id + 1)));
                },

                // In times 20..25, we test the various filter options:
                //  - two DS pods are created
                //  - a kube-system pod is created
                //  - a label-selector pod is created
                //
                // In the test below, all of these events should be filtered out
                (20, id) => {
                    let pod = test_daemonset_pod(7);
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Applied(pod)), (new_ts, id)));
                },
                (21, id) => {
                    let pod = test_daemonset_pod(8);
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Applied(pod)), (new_ts, id)));
                },
                (22, id) => {
                    let mut pod = test_pod(30);
                    pod.metadata.namespace = Some("kube-system".into());
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Applied(pod)), (new_ts, id)));
                },
                (23, id) => {
                    let pod = test_daemonset_pod(1);
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Deleted(pod)), (new_ts, id)));
                },
                (24, id) => {
                    let mut pod = test_pod(31);
                    pod.labels_mut().insert("foo".into(), "bar".into());
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Applied(pod)), (new_ts, id)));
                },

                // Lastly we delete the remaining "regular" pods
                (25..=55, id) => {
                    let pod = test_pod(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Deleted(pod)), (new_ts, id + 1)));
                },
                _ => None,
            }
        }
    })
    .boxed();
}

#[tokio::test]
async fn test_export() {
    // First build up the stream of test data and run the watcher (this advances time to the "end")
    let t = Tracer::new();
    let clock = Arc::new(Mutex::new(MockUtcClock { now: 0 }));
    let mut w = Watcher::new_from_parts(test_stream(clock.clone()), t.clone(), clock);
    w.start().await;

    // Next export the data with the chosen filters
    let filter = TraceFilter {
        excluded_namespaces: vec!["kube-system".into()],
        excluded_labels: vec![metav1::LabelSelector {
            match_labels: Some(BTreeMap::from([("foo".into(), "bar".into())])),
            ..Default::default()
        }],
        exclude_daemonsets: true,
    };

    let tracer = t.lock().unwrap();
    let (start_ts, end_ts) = (15, 46);
    match tracer.export(start_ts, end_ts, &filter) {
        Ok(data) => {
            // Confirm that the results match what we expect
            let new_tracer = Tracer::import(data).unwrap();
            let expected_pods = tracer.objs_at(end_ts, &filter);
            let actual_pods = new_tracer.objs();
            println!("Expected pods: {:?}", expected_pods);
            println!("Actual pods: {:?}", actual_pods);
            assert_eq!(expected_pods, actual_pods);
        },
        Err(e) => panic!("failed with {}", e),
    };
}
