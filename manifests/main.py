#!/usr/bin/env python
from cdk8s import App
from simkube import Simkube
from sk_cloudprov import ClusterAutoscaler
from sk_cloudprov import SKCloudProv
from test_deployment import TestDeployment


if __name__ == "__main__":
    app = App()
    namespace = "default"
    Simkube(app, namespace, "simkube")
    TestDeployment(app, namespace, "test")
    skprov = SKCloudProv(app, namespace, "sk-cloudprov")
    ca = ClusterAutoscaler(app, "cluster-autoscaler", skprov.get_grpc_address())
    ca.add_dependency(skprov)

    app.synth()
