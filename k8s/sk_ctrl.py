import os

import fireconfig as fire
from cdk8s import Chart
from constructs import Construct

ID = "sk-ctrl"


class SKController(Chart):
    def __init__(self, scope: Construct, namespace: str):
        super().__init__(scope, ID)

        app_key = "app"

        with open(os.getenv('BUILD_DIR') + f'/{ID}-image') as f:
            image = f.read()
        container = fire.ContainerBuilder(
            name=ID,
            image=image,
            command="/sk-ctrl",
        )

        depl = (fire.DeploymentBuilder(namespace=namespace, selector={app_key: ID})
            .with_label(app_key, ID)
            .with_service_account_and_role_binding('cluster-admin', True)
            .with_containers(container)
            .with_node_selector("type", "kind-worker")
        )
        depl.build(self)
