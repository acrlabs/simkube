use std::collections::HashMap;
use std::sync::{
    Arc,
    Mutex,
};

use httpmock::prelude::*;
use httpmock::{
    Mock,
    Then,
    When,
};
use serde_json::json;

#[derive(Clone)]
pub struct MockServerBuilder {
    server: Arc<Mutex<MockServer>>,
    mock_ids: Arc<Mutex<HashMap<usize, usize>>>,
}

impl MockServerBuilder {
    pub fn new() -> MockServerBuilder {
        let server = MockServer::start();
        MockServerBuilder {
            server: Arc::new(Mutex::new(server)),
            mock_ids: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn assert(&self) {
        for (id, calls) in self.mock_ids.lock().unwrap().iter() {
            println!("checking assertions for mock {id}; expected calls = {calls}");
            Mock::new(*id, &self.server.lock().unwrap()).assert_calls(*calls)
        }
    }

    pub fn drop(&mut self, mock_id: usize) {
        let calls = self.mock_ids.lock().unwrap().remove(&mock_id).unwrap();
        let server = &self.server.lock().unwrap();
        let mut mock = Mock::new(mock_id, server);
        println!("checking assertions for mock {mock_id}; expected calls = {calls}");
        mock.assert_calls(calls);
        mock.delete();
    }

    pub fn handle<F: Fn(When, Then) + 'static>(&mut self, f: F) -> usize {
        self.handle_multiple(f, 1)
    }

    pub fn handle_multiple<F: Fn(When, Then) + 'static>(&mut self, f: F, calls: usize) -> usize {
        let mock_id = self.server.lock().unwrap().mock(f).id;
        self.mock_ids.lock().unwrap().insert(mock_id, calls);
        mock_id
    }

    pub fn handle_not_found(&mut self, path: String) -> usize {
        self.handle(move |when, then| {
            when.path(&path);
            then.status(404).json_body(status_not_found());
        })
    }

    pub fn url(&self) -> http::Uri {
        http::Uri::try_from(self.server.lock().unwrap().url("/")).unwrap()
    }
}

pub fn make_fake_apiserver() -> (MockServerBuilder, kube::Client) {
    let builder = MockServerBuilder::new();
    let config = kube::Config::new(builder.url());
    let client = kube::Client::try_from(config).unwrap();
    (builder, client)
}

pub fn status_ok() -> serde_json::Value {
    json!({
      "kind": "Status",
      "apiVersion": "v1",
      "metadata": {},
      "status": "Success",
      "code": 200
    })
}

pub fn status_not_found() -> serde_json::Value {
    json!({
      "kind": "Status",
      "apiVersion": "v1",
      "metadata": {},
      "status": "Failure",
      "reason": "NotFound",
      "code": 404
    })
}

pub fn apps_v1_discovery() -> serde_json::Value {
    json!({
        "kind":"APIResourceList",
        "apiVersion":"v1",
        "groupVersion":"apps/v1",
        "resources":[
            {
                "name":"controllerrevisions",
                "singularName":"controllerrevision",
                "namespaced":true,
                "kind":"ControllerRevision",
                "verbs":["create","delete","deletecollection","get","list","patch","update","watch"],
                "storageVersionHash":"85nkx63pcBU=",
            },
            {
                "name":"daemonsets",
                "singularName":"daemonset",
                "namespaced":true,
                "kind":"DaemonSet",
                "verbs":["create","delete","deletecollection","get","list","patch","update","watch"],
                "shortNames":["ds"],
                "categories":["all"],
                "storageVersionHash":"dd7pWHUlMKQ=",
            },
            {
                "name":"daemonsets/status",
                "singularName":"",
                "namespaced":true,
                "kind":"DaemonSet",
                "verbs":["get","patch","update"],
            },
            {
                "name":"deployments",
                "singularName":"deployment",
                "namespaced":true,
                "kind":"Deployment",
                "verbs":["create","delete","deletecollection","get","list","patch","update","watch"],
                "shortNames":["deploy"],
                "categories":["all"],
                "storageVersionHash":"8aSe+NMegvE=",
            },
            {
                "name":"deployments/scale",
                "singularName":"",
                "namespaced":true,
                "group":"autoscaling",
                "version":"v1",
                "kind":"Scale",
                "verbs":["get","patch","update"],
            },
            {
                "name":"deployments/status",
                "singularName":"",
                "namespaced":true,
                "kind":"Deployment",
                "verbs":["get","patch","update"],
            },
            {
                "name":"replicasets",
                "singularName":"replicaset",
                "namespaced":true,
                "kind":"ReplicaSet",
                "verbs":["create","delete","deletecollection","get","list","patch","update","watch"],
                "shortNames":["rs"],
                "categories":["all"],
                "storageVersionHash":"P1RzHs8/mWQ=",
            },
            {
                "name":"replicasets/scale",
                "singularName":"",
                "namespaced":true,
                "group":"autoscaling",
                "version":"v1",
                "kind":"Scale",
                "verbs":["get","patch","update"],
            },
            {
                "name":"replicasets/status",
                "singularName":"",
                "namespaced":true,
                "kind":"ReplicaSet",
                "verbs":["get","patch","update"],
            },
            {
                "name":"statefulsets",
                "singularName":"statefulset",
                "namespaced":true,
                "kind":"StatefulSet",
                "verbs":["create","delete","deletecollection","get","list","patch","update","watch"],
                "shortNames":["sts"],
                "categories":["all"],
                "storageVersionHash":"H+vl74LkKdo=",
            },
            {
                "name":"statefulsets/scale",
                "singularName":"",
                "namespaced":true,
                "group":"autoscaling",
                "version":"v1",
                "kind":"Scale",
                "verbs":["get","patch","update"],
            },
            {
                "name":"statefulsets/status",
                "singularName":"",
                "namespaced":true,
                "kind":"StatefulSet",
                "verbs":["get","patch","update"],
            },
        ],
    })
}
