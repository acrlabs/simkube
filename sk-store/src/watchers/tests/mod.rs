mod pod_watcher_test;

use futures::stream;
use mockall::predicate;
use sk_core::prelude::*;
use sk_testutils::*;
use tokio::sync::mpsc;

use super::*;
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

    let (ready_tx, _): (mpsc::Sender<bool>, mpsc::Receiver<bool>) = mpsc::channel(1);
    let mut watcher = ObjWatcher::<DynamicObject>::new(handler, Box::pin(stream::empty()), ready_tx);

    watcher.handle_event(&Event::Init, 0).await.unwrap();
    for depl in deployments {
        watcher.handle_event(&Event::InitApply(depl), 0).await.unwrap();
    }
    watcher.handle_event(&Event::InitDone, 0).await.unwrap();
}
