import os

import fireconfig as fire
from cdk8s import Chart
from constructs import Construct

ID = "sk-tracer"
SERVER_PORT = 7777


class SKTracer(Chart):
    def __init__(self, scope: Construct, namespace: str):
        super().__init__(scope, ID)

        app_key = "app"

        with open(os.getenv('BUILD_DIR') + f'/{ID}-image') as f:
            image = f.read()
        container = fire.ContainerBuilder(
            name=ID,
            image=image,
            command="/sk-tracer",
            args=["--server-port", f"{SERVER_PORT}"],
        ).with_ports(SERVER_PORT)

        depl = (fire.DeploymentBuilder(namespace=namespace, selector={app_key: ID})
            .with_label(app_key, ID)
            .with_service_account_and_role_binding('cluster-admin', True)
            .with_containers(container)
            .with_service()
            .with_node_selector("type", "kind-worker")
        )
        depl.build(self)
