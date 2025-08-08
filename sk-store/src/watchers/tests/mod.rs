mod pod_watcher_test;

use assertables::*;
use futures::stream;
use lazy_static::lazy_static;
use mockall::predicate;
use sk_core::prelude::*;
use sk_testutils::*;
use tokio::sync::mpsc;

use super::*;
use crate::watchers::MockEventHandler;

lazy_static! {
    static ref EXPECTED_INDEX: HashSet<String> = HashSet::from([
        format!("{TEST_NAMESPACE}/depl0"),
        format!("{TEST_NAMESPACE}/depl1"),
        format!("{TEST_NAMESPACE}/depl2"),
    ]);
}

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

    assert_bag_eq!(watcher.index, *EXPECTED_INDEX);
}

#[rstest(tokio::test)]
async fn test_handle_initialize_event_with_created_obj() {
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
    watcher.index = HashSet::from([format!("{TEST_NAMESPACE}/depl0"), format!("{TEST_NAMESPACE}/depl1")]);

    watcher.handle_event(&Event::Init, 0).await.unwrap();
    for depl in deployments {
        watcher.handle_event(&Event::InitApply(depl), 0).await.unwrap();
    }
    watcher.handle_event(&Event::InitDone, 0).await.unwrap();

    assert_bag_eq!(watcher.index, *EXPECTED_INDEX);
}

#[rstest(tokio::test)]
async fn test_handle_initialize_event_with_deleted_obj() {
    let deployments: Vec<_> = (0..2).map(|i| test_deployment(&format!("depl{i}"))).collect();
    let mut handler = Box::new(MockEventHandler::new());
    for i in 0..2 {
        handler
            .expect_applied()
            .with(predicate::eq(deployments[i].clone()), predicate::eq(0))
            .returning(|_, _| Ok(()))
            .once();
    }

    handler
        .expect_deleted()
        .with(predicate::eq(format!("{TEST_NAMESPACE}/depl2")), predicate::eq(0))
        .returning(|_, _| Ok(()))
        .once();

    let (ready_tx, _): (mpsc::Sender<bool>, mpsc::Receiver<bool>) = mpsc::channel(1);
    let mut watcher = ObjWatcher::<DynamicObject>::new(handler, Box::pin(stream::empty()), ready_tx);
    watcher.index = EXPECTED_INDEX.clone();

    watcher.handle_event(&Event::Init, 0).await.unwrap();
    for depl in deployments {
        watcher.handle_event(&Event::InitApply(depl), 0).await.unwrap();
    }
    watcher.handle_event(&Event::InitDone, 0).await.unwrap();

    assert_bag_eq!(
        watcher.index,
        HashSet::from([format!("{TEST_NAMESPACE}/depl0"), format!("{TEST_NAMESPACE}/depl1"),])
    );
}
