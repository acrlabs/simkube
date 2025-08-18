use httpmock::Method::*;
use serde_json::json;

use super::*;

#[rstest(tokio::test)]
async fn test_manager_start_wait_ready() {
    let config_yml = "
---
trackedObjects:
  apps/v1.Deployment:
    podSpecTemplatePaths:
      - /foo/bar
"
    .to_string();

    let config: TracerConfig = serde_yaml::from_str(&config_yml).unwrap();
    let (mut fake_apiserver, client) = make_fake_apiserver();

    fake_apiserver.handle(|when, then| {
        when.path("/apis/apps/v1").method(GET);
        then.json_body(apps_v1_discovery());
    });

    // The limit query params indicate these are the initial "list" calls
    fake_apiserver.handle(|when, then| {
        when.path("/apis/apps/v1/deployments").method(GET).query_param("limit", "500");
        then.json_body(json!({
            "kind": "List",
            "apiVersion": "apps/v1",
            "items": [],
            "metadata": {"resourceVersion": "1"},
        }));
    });
    fake_apiserver.handle(|when, then| {
        when.path("/api/v1/pods").method(GET).query_param("limit", "500");
        then.json_body(json!({
            "kind": "List",
            "apiVersion": "v1",
            "items": [],
            "metadata": {"resourceVersion": "1"},
        }));
    });

    // The fake apiserver is going to throw a bunch of errors because it's not
    // getting any responses back from the watch call, but for the purposes of
    // this test I don't really care; at that point, errors are swallowed by the
    // ObjWatcher, so it doesn't shut anything down and I just want to test that
    // wait_ready works.
    //
    // In the future if you _do_ want to test responses to the watch call, you
    // would filter on the watch=true query_param.

    let mut manager = TraceManager::start(client, config).await.unwrap();
    manager.wait_ready().await;
    manager.shutdown().await;
    fake_apiserver.assert();
}
