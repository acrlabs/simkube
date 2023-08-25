#!/usr/bin/env python
from cdk8s import App
from sk_cloudprov import ClusterAutoscaler
from sk_cloudprov import SKCloudProv
from sk_vnode import SKVnode
from test_deployment import TestDeployment


if __name__ == "__main__":
    app = App()
    namespace = "default"

    SKVnode(app, namespace)
    TestDeployment(app, namespace)
    skprov = SKCloudProv(app, namespace)
    ca = ClusterAutoscaler(app, skprov.get_grpc_address())
    ca.add_dependency(skprov)

    app.synth()
