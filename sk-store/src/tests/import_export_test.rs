use std::collections::HashSet;
use std::sync::Arc;

use assertables::*;
use clockabilly::Clockable;
use clockabilly::mock::MockUtcClock;
use futures::stream;
use futures::stream::StreamExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::ResourceExt;
use kube::discovery::ApiResource;
use kube::runtime::watcher::Event;
use serde_json::json;
use sk_api::v1::ExportFilters;
use sk_core::k8s::{
    DynamicApiSet,
    GVK,
    format_gvk_name,
};
use sk_core::macros::*;
use tokio::sync::{
    Mutex,
    mpsc,
};

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

fn rs(idx: i64) -> DynamicObject {
    let mut obj = DynamicObject::new(&format!("repset{idx}"), &ApiResource::from_gvk(&REPLICASET_GVK))
        .within(TEST_NAMESPACE)
        .data(json!({"spec": {"replicas": 42}}));

    obj.owner_references_mut().push(metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: format!("depl{idx}"),
        ..Default::default()
    });

    obj
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
        println!("current state machine state: {state:?}");
        let mut c = clock.clone();
        async move {
            match state {
                // Initial conditions: we create 10 deployments; each deployment owns a replicaset
                (-1, -1) => {
                    return Some((Ok(Event::Init), (-1, 0)));
                },

                (-1, id) if id < 10 => {
                    let obj = d(id);
                    return Some((Ok(Event::InitApply(obj)), (-1, id + 1)));
                },

                // We expect none of the replicasets to be exported because they are all owned
                // by a deployment above
                (-1, id) if id < 20 => {
                    let obj = rs(id - 10);
                    return Some((Ok(Event::InitApply(obj)), (-1, id + 1)));
                },

                (-1, 20) => {
                    return Some((Ok(Event::InitDone), (0, 0)));
                },

                // We recreate one of the deployments at time zero just to make sure there's
                // no weird duplicate behaviours
                (0, id) => {
                    let obj = d(id);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Apply(obj)), (new_ts, id)));
                },

                // From times 10..20, we delete one of the regular deployments...
                (5..=19, id) if id < 10 => {
                    let obj = d(id);
                    let curr_ts = c.now_ts();
                    return Some((Ok(Event::Delete(obj)), (curr_ts, id + 10)));
                },

                // ... and the replicaset it owns
                (5..=19, id) if id >= 10 => {
                    let obj = rs(id - 10);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Delete(obj)), (new_ts, id - 9)));
                },

                // In times 20..25, we test the various filter options:
                //  - a kube-system deployment is created
                //  - a label-selector deployment is created
                //
                // In the test below, all of these events should be filtered out
                (20, id) => {
                    let mut obj = d(30);
                    obj.metadata.namespace = Some("kube-system".into());
                    let new_ts = c.advance(4);
                    return Some((Ok(Event::Apply(obj)), (new_ts, id)));
                },
                (24, id) => {
                    let mut obj = d(31);
                    obj.labels_mut().insert("foo".into(), "bar".into());
                    let new_ts = c.advance(1);
                    return Some((Ok(Event::Apply(obj)), (new_ts, id)));
                },

                // Lastly we delete the remaining "regular" deployments...
                (25..=55, id) if id < 10 => {
                    let obj = d(id);
                    let curr_ts = c.now_ts();
                    return Some((Ok(Event::Delete(obj)), (curr_ts, id + 10)));
                },

                // ... and their replicasets
                (25..=55, id) if id >= 10 => {
                    let obj = rs(id - 10);
                    let new_ts = c.advance(5);
                    return Some((Ok(Event::Delete(obj)), (new_ts, id - 9)));
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
        let (mut fake_apiserver, client) = make_fake_apiserver();
        let apiset = DynamicApiSet::new(client);

        fake_apiserver.handle(|when, then| {
            when.path("/apis/apps/v1");
            then.json_body(apps_v1_discovery());
        });

        for i in 0..10 {
            fake_apiserver.handle(move |when, then| {
                when.path("/apis/apps/v1/deployments")
                    .query_param("fieldSelector", format!("metadata.namespace={TEST_NAMESPACE},metadata.name=depl{i}"));
                then.json_body(json!({
                    "metadata": {},
                    "items": [
                        {
                            "metadata": {
                                "namespace": TEST_NAMESPACE,
                                "name": format!("depl{i}"),
                            }
                        },
                    ],
                }));
            });
        }

        // We're essentially duplicating the work of the TraceManager here, but it is so finnicky
        // about ownership and stuff that I don't see a clean way to replace this code with the
        // TraceManager code right now
        let s = Arc::new(Mutex::new(TraceStore::new(config.clone(), apiset)));
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
        let store = s.lock().await;
        match store.export(start_ts, end_ts, &filter).await {
            Ok(data) => {
                // Confirm that the results match what we expect
                let trace = ExportedTrace::import(data, duration.as_ref()).unwrap();
                let import_end_ts = duration.map(|_| start_ts + 10).unwrap_or(end_ts);
                let expected_objs = store.objs_at(import_end_ts, &filter).await;
                let actual_objs = objs_in_trace(&trace);

                println!("{actual_objs:?}");
                assert_bag_eq!(actual_objs, expected_objs);
                for obj in actual_objs {
                    assert_not_contains!(obj, "depl30"); // kube-system namespace
                    assert_not_contains!(obj, "depl31"); // label-selector
                    assert_not_contains!(obj, "repset"); // owned objects
                }
            },
            Err(e) => panic!("failed with error: {}", e),
        };
    }
}
