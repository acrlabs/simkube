use httpmock::prelude::*;
use httpmock::{
    Mock,
    Then,
    When,
};
use serde_json::json;

pub struct MockServerBuilder {
    server: MockServer,
    handlers: Vec<Box<dyn Fn(When, Then)>>,
    mock_ids: Vec<usize>,
}

fn print_req(req: &HttpMockRequest) -> bool {
    // Use println instead of info! so that this works outside of the lib crate
    println!("    Received: {} {}", req.method, req.path);
    true
}

impl MockServerBuilder {
    pub fn new() -> MockServerBuilder {
        MockServerBuilder {
            server: MockServer::start(),
            handlers: vec![],
            mock_ids: vec![],
        }
    }

    pub fn assert(&self) {
        for id in &self.mock_ids {
            println!("checking assertions for mock {id}");
            Mock::new(*id, &self.server).assert()
        }
    }

    pub fn handle<F: Fn(When, Then) + 'static>(&mut self, f: F) -> &mut Self {
        self.handlers.push(Box::new(move |w, t| {
            let w = w.matches(print_req);
            f(w, t);
        }));
        self
    }

    pub fn handle_not_found(&mut self, path: String) -> &mut Self {
        self.handle(move |when, then| {
            when.path(&path);
            then.status(404).json_body(status_not_found());
        })
    }

    pub fn build(&mut self) {
        for f in self.handlers.iter() {
            self.mock_ids.push(self.server.mock(f).id);
        }

        // Print all unmatched/unhandled requests for easier debugging;
        // this has to go last so that the other mock rules have a chance
        // to match first
        self.server.mock(|when, _| {
            when.matches(print_req);
        });
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
