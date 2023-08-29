use std::collections::VecDeque;
use std::sync::{
    Arc,
    Mutex,
};

use futures::stream;
use futures::stream::StreamExt;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::runtime::watcher::Event;
use simkube::util::Clockable;
use simkube::watchertracer::{
    PodStream,
    TraceEvent,
    Tracer,
    Watcher,
};

const TESTING_NAMESPACE: &str = "test";

struct MockUtcClock {
    now: i64,
}

impl MockUtcClock {
    fn advance(&mut self, duration: i64) {
        self.now += duration;
    }
}

impl Clockable for MockUtcClock {
    fn now(&self) -> i64 {
        return self.now;
    }
}

fn test_pod(idx: i64) -> corev1::Pod {
    return corev1::Pod {
        metadata: metav1::ObjectMeta {
            namespace: Some(TESTING_NAMESPACE.into()),
            name: Some(format!("pod{}", idx).into()),
            ..metav1::ObjectMeta::default()
        },
        ..corev1::Pod::default()
    };
}

// Set up a test stream to ensure that imports and exports work correctly.
// The test stream looks like this:
//
//   - at time 0, ten pods are created (initial conditions)
//   - at time 5/10/15/..., one of the original ten pods is deleted
//
// If you start your export at (say) time 13, you can check whether the correct
// pods exist in the export's setup/initial conditions (i.e., pod1 and pod2 should
// be deleted in this scenario, but pod3..9 should be present).
fn test_stream(clock: Arc<Mutex<MockUtcClock>>) -> PodStream {
    return stream::unfold(0, move |idx| {
        let clock = clock.clone();
        async move {
            if idx < 10 {
                let pod = test_pod(idx);
                return Some((Ok(Event::Applied(pod)), idx + 1));
            } else if idx < 20 {
                clock.lock().unwrap().advance(5);
                let pod = test_pod(idx - 10);
                return Some((Ok(Event::Deleted(pod)), idx + 1));
            } else {
                None
            }
        }
    })
    .boxed();
}

#[tokio::test]
async fn test_export() {
    let t = Tracer::new();
    let clock = Arc::new(Mutex::new(MockUtcClock { now: 0 }));
    let mut w = Watcher::new_from_parts(test_stream(clock.clone()), t.clone(), clock);
    w.start().await;

    let end: i64 = 30;
    let start: i64 = 15;
    let tracer = t.lock().unwrap();
    match tracer.export(start, end) {
        Ok(data) => {
            let new_tracer = Tracer::import(data).unwrap();
            assert_eq!(tracer.pods_at(end), new_tracer.pods());
        },
        Err(e) => panic!("failed with {}", e),
    };
}
