use super::*;

mod itest {
    use insta::assert_snapshot;
    use serde_json::json;
    use sk_core::k8s::DynamicApiSet;

    use super::*;

    #[rstest(tokio::test)]
    #[case::no_type(None)]
    #[case::some_type(Some(POD_GVK.into_type_meta()))]
    async fn test_dyn_obj_watcher_sanitize_pod_obj(#[case] types: Option<TypeMeta>) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        let mut apiset = DynamicApiSet::new(client);

        fake_apiserver.handle(|when, then| {
            when.path("/api/v1");
            then.json_body(v1_discovery());
        });

        let new_types = types.clone();
        fake_apiserver.handle(move |when, then| {
            when.path("/api/v1/pods").query_param_missing("watch");
            then.json_body(json!({
                "metadata": {
                    "resourceVersion": "42",
                },
                "items": [
                    DynamicObject {
                        metadata: metav1::ObjectMeta {
                            name: Some("test-obj".into()),
                            namespace: Some(TEST_NAMESPACE.into()),
                            ..Default::default()
                        },
                        types: new_types.clone(),
                        data: json!({
                            "spec": {
                                "nodeName": "ip-1-2-3-4.internal",
                            }
                        }),
                    }
                ],
            }));
        });

        let (dyn_obj_tx, mut dyn_obj_rx): (dyn_obj_watcher::Sender, dyn_obj_watcher::Receiver) =
            mpsc::unbounded_channel();
        let (ready_tx, _): (mpsc::Sender<bool>, mpsc::Receiver<bool>) = mpsc::channel(1);
        let mut w = dyn_obj_watcher::new_with_stream(&*POD_GVK, &mut apiset, dyn_obj_tx, ready_tx)
            .await
            .unwrap();
        w.handle_next_event().await.unwrap(); // Init event
        w.handle_next_event().await.unwrap(); // InitApply event with the pod
        w.handle_next_event().await.unwrap(); // InitDone event; ship everything to the channel
        fake_apiserver.assert();

        let received_message = dyn_obj_rx.recv().await.unwrap();
        assert_snapshot!(serde_yaml::to_string(&received_message.obj).unwrap());
    }
}
