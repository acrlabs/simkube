mod pod_watcher_test;

use futures::stream;
use mockall::predicate;
use sk_core::prelude::*;
use sk_testutils::*;

use super::*;
use crate::mock::MockTraceStore;
use crate::watchers::MockEventHandler;

#[rstest(tokio::test)]
async fn test_handle_initialize_event() {
    let deployments: Vec<_> = (0..3).map(|i| test_deployment(&format!("depl{i}"))).collect();
    let mut handler = Box::new(MockEventHandler::new());
    for i in 0..3 {
        handler
            .expect_applied()
            .with(predicate::eq(deployments[i].clone()), predicate::eq(0))
            .returning(|_, _| Ok(()))
            .once();
    }

    let (mut watcher, _) = ObjWatcher::<DynamicObject>::new(handler, Box::pin(stream::empty()));

    watcher.handle_event(&Event::Init, 0).await.unwrap();
    for depl in deployments {
        watcher.handle_event(&Event::InitApply(depl), 0).await.unwrap();
    }
    watcher.handle_event(&Event::InitDone, 0).await.unwrap();
}
