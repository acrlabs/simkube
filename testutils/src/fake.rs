use httpmock::prelude::*;
use httpmock::{
    Mock,
    Then,
    When,
};
use serde_json::json;

pub struct MockServerBuilder {
    server: MockServer,
    mock_ids: Vec<(usize, usize)>,
}

impl MockServerBuilder {
    pub fn new() -> MockServerBuilder {
        MockServerBuilder { server: MockServer::start(), mock_ids: vec![] }
    }

    pub fn assert(&self) {
        for (id, calls) in &self.mock_ids {
            println!("checking assertions for mock {id}");
            Mock::new(*id, &self.server).assert_calls(*calls)
        }
    }

    pub fn handle<F: Fn(When, Then) + 'static>(&mut self, f: F) -> usize {
        self.handle_multiple(f, 1)
    }

    pub fn handle_multiple<F: Fn(When, Then) + 'static>(&mut self, f: F, calls: usize) -> usize {
        let mock_id = self.server.mock(f).id;
        self.mock_ids.push((mock_id, calls));
        mock_id
    }

    pub fn handle_not_found(&mut self, path: String) -> usize {
        self.handle(move |when, then| {
            when.path(&path);
            then.status(404).json_body(status_not_found());
        })
    }

    pub fn url(&self) -> http::Uri {
        http::Uri::try_from(self.server.url("/")).unwrap()
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
