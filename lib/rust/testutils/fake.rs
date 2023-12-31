use httpmock::prelude::*;
use httpmock::{
    Then,
    When,
};
use serde_json::json;
use tracing::*;

use crate::k8s::ApiSet;

pub struct MockServerBuilder {
    server: MockServer,
    handlers: Vec<Box<dyn Fn(When, Then)>>,
}

fn print_req(req: &HttpMockRequest) -> bool {
    info!("Received: {req:?}");
    true
}

impl MockServerBuilder {
    pub fn new() -> MockServerBuilder {
        MockServerBuilder { server: MockServer::start(), handlers: vec![] }
    }

    pub fn handle<F: Fn(When, Then) + 'static>(&mut self, f: F) -> &mut Self {
        self.handlers.push(Box::new(move |w, t| {
            let w = w.matches(print_req);
            f(w, t);
        }));
        self
    }

    pub fn build(&self) {
        for f in self.handlers.iter() {
            self.server.mock(f);
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

pub fn make_fake_apiserver() -> (MockServerBuilder, ApiSet) {
    let builder = MockServerBuilder::new();
    let config = kube::Config {
        cluster_url: builder.url(),
        default_namespace: "default".into(),
        root_cert: None,
        connect_timeout: None,
        read_timeout: None,
        write_timeout: None,
        accept_invalid_certs: true,
        auth_info: Default::default(),
        proxy_url: None,
        tls_server_name: None,
    };

    let client = kube::Client::try_from(config).unwrap();
    (builder, ApiSet::new(client))
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
